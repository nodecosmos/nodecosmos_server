mod callbacks;
pub mod flow_steps_by_index;

use crate::errors::NodecosmosError;
use crate::models::flow_step::flow_steps_by_index::FlowStepsByIndex;
use crate::models::helpers::ClonedRef;
use crate::models::input_output::InputOutput;
use crate::models::workflow::Workflow;
use charybdis::macros::charybdis_model;
use charybdis::operations::{Find, New, UpdateWithCallbacks};
use charybdis::types::{Double, Frozen, List, Map, Text, Timestamp, Uuid};
use scylla::CachingSession;
use serde::{Deserialize, Serialize};

#[charybdis_model(
    table_name = flow_steps,
    partition_keys = [node_id],
    clustering_keys = [workflow_id, flow_id, flow_index, id],
    global_secondary_indexes = [],
    local_secondary_indexes = [
        ([node_id], [id])
    ]
)]
#[derive(Serialize, Deserialize, Default)]
pub struct FlowStep {
    #[serde(rename = "nodeId")]
    pub node_id: Uuid,

    #[serde(rename = "workflowId")]
    pub workflow_id: Uuid,

    #[serde(rename = "flowId")]
    pub flow_id: Uuid,

    #[serde(rename = "flowIndex")]
    pub flow_index: Double,

    #[serde(default = "Uuid::new_v4")]
    pub id: Uuid,

    pub description: Option<Text>,

    #[serde(rename = "descriptionMarkdown")]
    pub description_markdown: Option<Text>,

    #[serde(rename = "nodeIds")]
    pub node_ids: Option<List<Uuid>>,

    #[serde(rename = "inputIdsByNodeId")]
    pub input_ids_by_node_id: Option<Frozen<Map<Uuid, Frozen<List<Uuid>>>>>,

    #[serde(rename = "outputIdsByNodeId")]
    pub output_ids_by_node_id: Option<Frozen<Map<Uuid, Frozen<List<Uuid>>>>>,

    #[serde(rename = "prevFlowStepId")]
    pub prev_flow_step_id: Option<Uuid>,

    #[serde(rename = "nextFlowStepId")]
    pub next_flow_step_id: Option<Uuid>,

    #[serde(rename = "createdAt")]
    pub created_at: Option<Timestamp>,

    #[serde(rename = "updatedAt")]
    pub updated_at: Option<Timestamp>,
}

impl FlowStep {
    pub async fn find_by_node_id_and_id(
        session: &CachingSession,
        node_id: Uuid,
        id: Uuid,
    ) -> Result<FlowStep, NodecosmosError> {
        let fs = find_one_flow_step!(session, "node_id = ? AND id = ?", (node_id, id)).await?;

        Ok(fs)
    }

    pub async fn prev_flow_step(&self, session: &CachingSession) -> Result<Option<FlowStep>, NodecosmosError> {
        if let Some(prev_flow_step_id) = self.prev_flow_step_id {
            let res = FlowStep::find_by_node_id_and_id(session, self.node_id, prev_flow_step_id).await?;

            Ok(Some(res))
        } else {
            Ok(None)
        }
    }

    pub async fn next_flow_step(&self, session: &CachingSession) -> Result<Option<FlowStep>, NodecosmosError> {
        if let Some(next_flow_step_id) = self.next_flow_step_id {
            let res = FlowStep::find_by_node_id_and_id(session, self.node_id, next_flow_step_id).await?;

            Ok(Some(res))
        } else {
            Ok(None)
        }
    }

    pub async fn pull_output_id(&mut self, session: &CachingSession, output_id: Uuid) -> Result<(), NodecosmosError> {
        if let Some(output_ids_by_node_id) = self.output_ids_by_node_id.as_mut() {
            for (_, input_ids) in output_ids_by_node_id.iter_mut() {
                input_ids.retain(|id| id != &output_id);
            }

            self.update_cb(session).await?;
        }

        Ok(())
    }

    pub async fn pull_input_id(&mut self, session: &CachingSession, input_id: Uuid) -> Result<(), NodecosmosError> {
        if let Some(input_ids_by_node_id) = self.input_ids_by_node_id.as_mut() {
            for (_, input_ids) in input_ids_by_node_id.iter_mut() {
                input_ids.retain(|id| id != &input_id);
            }

            self.update_cb(session).await?;
        }

        Ok(())
    }

    pub async fn remove_inputs(&mut self, session: &CachingSession) -> Result<(), NodecosmosError> {
        self.input_ids_by_node_id = None;
        self.update_cb(session).await?;

        Ok(())
    }

    async fn delete_fs_outputs(
        &mut self,
        session: &CachingSession,
        workflow: &Workflow,
    ) -> Result<(), NodecosmosError> {
        if let Some(output_ids_by_node_id) = &self.output_ids_by_node_id.clone() {
            let output_ids = output_ids_by_node_id.values().flatten().cloned().collect::<Vec<Uuid>>();
            InputOutput::delete_by_ids(session, output_ids.clone(), workflow, self.node_id, Some(&self)).await?;
        }

        Ok(())
    }

    async fn delete_outputs_from_removed_nodes(
        &mut self,
        session: &CachingSession,
        workflow: &Workflow,
    ) -> Result<(), NodecosmosError> {
        let node_ids = self.node_ids.cloned_ref();
        let mut node_ids_to_remove = vec![];

        // delete outputs from db
        if let Some(output_ids_by_node_id) = self.output_ids_by_node_id.as_ref() {
            for (node_id, output_ids) in output_ids_by_node_id.iter() {
                if !node_ids.contains(node_id) {
                    InputOutput::delete_by_ids(session, output_ids.clone(), workflow, self.node_id, Some(self)).await?;

                    node_ids_to_remove.push(*node_id);
                }
            }
        }

        // delete outputs from output_ids_by_node_id
        if let Some(output_ids_by_node_id) = self.output_ids_by_node_id.as_mut() {
            for node_id in node_ids_to_remove {
                output_ids_by_node_id.remove(&node_id);
            }
            self.update_cb(session).await?;
        }

        Ok(())
    }

    async fn remove_inputs_from_removed_nodes(&mut self, session: &CachingSession) -> Result<(), NodecosmosError> {
        let node_ids = self.node_ids.cloned_ref();

        if let Some(input_ids_by_node_id) = self.input_ids_by_node_id.as_mut() {
            input_ids_by_node_id.retain(|node_id, _| node_ids.contains(node_id));
            self.update_cb(session).await?;
        }

        Ok(())
    }

    async fn workflow(&self, session: &CachingSession) -> Result<Workflow, NodecosmosError> {
        let mut workflow = Workflow::new();

        workflow.node_id = self.node_id;
        workflow.id = self.workflow_id;

        workflow.find_by_primary_key(session).await?;

        Ok(workflow)
    }

    // removes outputs as inputs from next workflow step
    async fn remove_outputs_from_next_wf_step(
        &self,
        session: &CachingSession,
        workflow: &Workflow,
    ) -> Result<(), NodecosmosError> {
        if let Some(output_ids_by_node_id) = &self.output_ids_by_node_id {
            let mut flow_steps_by_index = FlowStepsByIndex::build(session, &workflow).await?;
            let output_ids = output_ids_by_node_id.values().flatten().cloned().collect::<Vec<Uuid>>();

            for id in output_ids {
                let mut output = InputOutput::new();
                output.root_node_id = workflow.root_node_id;
                output.node_id = self.node_id;
                output.workflow_id = self.workflow_id;
                output.id = id;

                output
                    .remove_from_next_workflow_step(session, Some(self), &mut flow_steps_by_index)
                    .await?;
            }
        }

        Ok(())
    }
}

partial_flow_step!(
    UpdateInputIdsFlowStep,
    node_id,
    workflow_id,
    flow_id,
    flow_index,
    id,
    input_ids_by_node_id,
    updated_at
);

partial_flow_step!(
    UpdateOutputIdsFlowStep,
    node_id,
    workflow_id,
    flow_id,
    flow_index,
    id,
    output_ids_by_node_id,
    updated_at
);

partial_flow_step!(
    UpdateFlowStepNodeIds,
    node_id,
    workflow_id,
    flow_id,
    flow_index,
    id,
    node_ids,
    updated_at
);

partial_flow_step!(
    UpdateDescriptionFlowStep,
    node_id,
    workflow_id,
    flow_id,
    flow_index,
    id,
    description,
    description_markdown,
    updated_at
);

partial_flow_step!(DeleteFlowStep, node_id, workflow_id, flow_id, flow_index, id);
