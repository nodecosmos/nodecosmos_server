pub mod flow_steps_by_index;

use crate::errors::NodecosmosError;
use crate::models::flow_step::flow_steps_by_index::FlowStepsByIndex;
use crate::models::helpers::{
    created_at_cb_fn, impl_updated_at_cb, sanitize_description_cb, updated_at_cb_fn, ClonedRef,
};
use crate::models::input_output::InputOutput;
use crate::models::workflow::Workflow;
use charybdis::batch::CharybdisModelBatch;
use charybdis::callbacks::Callbacks;
use charybdis::macros::charybdis_model;
use charybdis::model::AsNative;
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

    async fn delete_fs_outputs(&mut self, session: &CachingSession) -> Result<(), NodecosmosError> {
        if let Some(output_ids_by_node_id) = &self.output_ids_by_node_id.clone() {
            let output_ids = output_ids_by_node_id.values().flatten().cloned().collect::<Vec<Uuid>>();
            self.delete_outputs_by_ids(session, &output_ids).await?;
        }

        Ok(())
    }

    async fn delete_outputs_from_removed_nodes(&mut self, session: &CachingSession) -> Result<(), NodecosmosError> {
        let node_ids = self.node_ids.cloned_ref();
        let mut node_ids_to_remove = vec![];

        // delete outputs from db
        if let Some(output_ids_by_node_id) = self.output_ids_by_node_id.as_ref() {
            for (node_id, output_ids) in output_ids_by_node_id.iter() {
                if !node_ids.contains(node_id) {
                    self.delete_outputs_by_ids(session, output_ids).await?;

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

    async fn delete_outputs_by_ids(&self, session: &CachingSession, ids: &Vec<Uuid>) -> Result<(), NodecosmosError> {
        let mut batch = CharybdisModelBatch::new();

        for output_id in ids.iter() {
            let mut output = InputOutput::new();

            output.node_id = self.node_id;
            output.workflow_id = self.workflow_id;
            output.flow_step_id = Some(self.id);
            output.id = *output_id;

            batch.append_delete(&output)?;
        }

        batch.execute(session).await?;

        Ok(())
    }

    async fn workflow(&self, session: &CachingSession) -> Result<Workflow, NodecosmosError> {
        let mut workflow = Workflow::new();

        workflow.node_id = self.node_id;
        workflow.id = self.workflow_id;

        workflow.find_by_primary_key(session).await?;

        Ok(workflow)
    }

    async fn remove_outputs_from_next_wf_step(&self, session: &CachingSession) -> Result<(), NodecosmosError> {
        let workflow = self.workflow(session).await?;
        let mut flow_steps_by_index = FlowStepsByIndex::build(session, &workflow).await?;
        if let Some(output_ids_by_node_id) = &self.output_ids_by_node_id {
            let output_ids = output_ids_by_node_id.values().flatten().cloned().collect::<Vec<Uuid>>();

            for id in output_ids {
                let mut output = InputOutput::new();
                output.root_node_id = workflow.root_node_id;
                output.node_id = self.node_id;
                output.workflow_id = self.workflow_id;
                output.id = id;

                output
                    .remove_from_next_workflow_step(session, self, &mut flow_steps_by_index)
                    .await?;
            }
        }

        Ok(())
    }
}

impl Callbacks<NodecosmosError> for FlowStep {
    created_at_cb_fn!();

    async fn after_insert(&self, session: &CachingSession) -> Result<(), NodecosmosError> {
        match (&self.prev_flow_step_id, &self.next_flow_step_id) {
            (Some(prev_fs_id), Some(next_fs_id)) => {
                let mut prev_fs = FlowStep::find_by_node_id_and_id(session, self.node_id, *prev_fs_id).await?;
                let mut next_fs = FlowStep::find_by_node_id_and_id(session, self.node_id, *next_fs_id).await?;

                prev_fs.remove_outputs_from_next_wf_step(session).await?;
                prev_fs.next_flow_step_id = Some(self.id);
                prev_fs.update_cb(session).await?;

                next_fs.prev_flow_step_id = Some(self.id);
                next_fs.update_cb(session).await?;
                next_fs.remove_inputs(session).await?;
            }
            (Some(prev_fs_id), None) => {
                let mut prev_fs = FlowStep::find_by_node_id_and_id(session, self.node_id, *prev_fs_id).await?;

                prev_fs.next_flow_step_id = Some(self.id);
                prev_fs.update_cb(session).await?;
            }
            (None, Some(next_fs_id)) => {
                let mut next_fs = FlowStep::find_by_node_id_and_id(session, self.node_id, *next_fs_id).await?;

                next_fs.prev_flow_step_id = Some(self.id);
                next_fs.update_cb(session).await?;
                next_fs.remove_inputs(session).await?;
            }
            _ => {}
        }

        Ok(())
    }

    updated_at_cb_fn!();

    async fn before_delete(&mut self, session: &CachingSession) -> Result<(), NodecosmosError> {
        self.delete_fs_outputs(session).await?;
        self.remove_outputs_from_next_wf_step(session).await?;

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

impl Callbacks<NodecosmosError> for UpdateInputIdsFlowStep {
    updated_at_cb_fn!();
}

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
impl_updated_at_cb!(UpdateOutputIdsFlowStep);

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

impl Callbacks<NodecosmosError> for UpdateFlowStepNodeIds {
    updated_at_cb_fn!();

    async fn after_update(&self, session: &CachingSession) -> Result<(), NodecosmosError> {
        let mut flow_step = self.as_native().find_by_primary_key(session).await?;

        flow_step.delete_outputs_from_removed_nodes(session).await?;
        flow_step.remove_inputs_from_removed_nodes(session).await?;

        Ok(())
    }
}

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
sanitize_description_cb!(UpdateDescriptionFlowStep);

partial_flow_step!(DeleteFlowStep, node_id, workflow_id, flow_id, flow_index, id);
