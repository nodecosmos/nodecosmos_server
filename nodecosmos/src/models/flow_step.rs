use crate::errors::NodecosmosError;
use crate::models::flow::Flow;
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
use charybdis::stream::CharybdisModelStream;
use charybdis::types::{Frozen, Int, List, Map, Text, Timestamp, Uuid};
use futures::TryStreamExt;
use scylla::CachingSession;
use serde::{Deserialize, Serialize};

#[charybdis_model(
    table_name = flow_steps,
    partition_keys = [node_id],
    clustering_keys = [workflow_id, workflow_index, id],
    global_secondary_indexes = [],
    local_secondary_indexes = [{
      "pk": ["node_id"],
      "ck": ["id"]
    }]
)]
#[derive(Serialize, Deserialize, Default)]
pub struct FlowStep {
    #[serde(rename = "nodeId")]
    pub node_id: Uuid,

    #[serde(rename = "workflowId")]
    pub workflow_id: Uuid,

    #[serde(rename = "workflowIndex")]
    pub workflow_index: Int,

    #[serde(default = "Uuid::new_v4")]
    pub id: Uuid,

    #[serde(rename = "flowId")]
    pub flow_id: Uuid,

    #[serde(rename = "nodeIds")]
    pub node_ids: Option<List<Uuid>>,

    pub description: Option<Text>,

    #[serde(rename = "descriptionMarkdown")]
    pub description_markdown: Option<Text>,

    #[serde(rename = "inputIdsByNodeId")]
    pub input_ids_by_node_id: Option<Frozen<Map<Uuid, Frozen<List<Uuid>>>>>,

    #[serde(rename = "outputIdsByNodeId")]
    pub output_ids_by_node_id: Option<Frozen<Map<Uuid, Frozen<List<Uuid>>>>>,

    #[serde(rename = "createdAt")]
    pub created_at: Option<Timestamp>,

    #[serde(rename = "updatedAt")]
    pub updated_at: Option<Timestamp>,
}

impl FlowStep {
    pub async fn flow_steps_by_workflow_index(
        session: &CachingSession,
        workflow: &Workflow,
        index: Int,
    ) -> Result<CharybdisModelStream<FlowStep>, NodecosmosError> {
        let res =
            FlowStep::find_by_node_id_and_workflow_id_and_workflow_index(session, workflow.node_id, workflow.id, index)
                .await?;

        Ok(res)
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

    async fn disassociate_outputs_from_subsequent_step(
        &self,
        session: &CachingSession,
        output_ids: &Vec<Uuid>,
    ) -> Result<(), NodecosmosError> {
        let mut next_flow_steps = FlowStep::find_by_node_id_and_workflow_id_and_workflow_index(
            session,
            self.node_id,
            self.workflow_id,
            self.workflow_index + 1,
        )
        .await?;

        while let Some(mut fs) = next_flow_steps.try_next().await? {
            for output_id in output_ids.iter() {
                fs.pull_input_id(session, *output_id).await?;
            }
        }

        Ok(())
    }
}

impl Callbacks<NodecosmosError> for FlowStep {
    created_at_cb_fn!();

    async fn after_insert(&mut self, session: &CachingSession) -> Result<(), NodecosmosError> {
        let mut flow = Flow::new();

        flow.node_id = self.node_id;
        flow.workflow_id = self.workflow_id;
        flow.id = self.flow_id;

        flow.append_step(session, self.id).await?;

        Ok(())
    }

    updated_at_cb_fn!();

    async fn before_delete(&mut self, session: &CachingSession) -> Result<(), NodecosmosError> {
        let mut flow = Flow::new();

        flow.node_id = self.node_id;
        flow.workflow_id = self.workflow_id;
        flow.id = self.flow_id;

        flow.remove_step(session, self.id).await?;
        self.delete_fs_outputs(session).await?;

        Ok(())
    }
}

partial_flow_step!(
    UpdateInputIdsFlowStep,
    node_id,
    workflow_id,
    workflow_index,
    flow_id,
    id,
    input_ids_by_node_id,
    updated_at
);
impl_updated_at_cb!(UpdateInputIdsFlowStep);

partial_flow_step!(
    UpdateOutputIdsFlowStep,
    node_id,
    workflow_id,
    workflow_index,
    flow_id,
    id,
    output_ids_by_node_id,
    updated_at
);
impl_updated_at_cb!(UpdateOutputIdsFlowStep);

partial_flow_step!(
    UpdateFlowStepNodeIds,
    node_id,
    workflow_id,
    workflow_index,
    flow_id,
    id,
    node_ids,
    updated_at
);

impl Callbacks<NodecosmosError> for UpdateFlowStepNodeIds {
    updated_at_cb_fn!();

    async fn after_update(&mut self, session: &CachingSession) -> Result<(), NodecosmosError> {
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
    workflow_index,
    id,
    description,
    description_markdown,
    updated_at
);
sanitize_description_cb!(UpdateDescriptionFlowStep);

partial_flow_step!(UpdateWorkflowIndexFlowStep, node_id, workflow_id, workflow_index, id);

partial_flow_step!(DeleteFlowStep, node_id, workflow_id, workflow_index, id);
