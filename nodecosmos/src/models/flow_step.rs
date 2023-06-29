use crate::models::flow::Flow;
use crate::models::helpers::{
    created_at_cb_fn, impl_updated_at_cb, sanitize_description_cb, updated_at_cb_fn,
};
use crate::models::input_output::InputOutput;
use charybdis::{
    charybdis_model, partial_model_generator, AsNative, Callbacks, CharybdisError,
    DeleteWithCallbacks, Find, Frozen, List, Map, New, Text, Timestamp, UpdateWithCallbacks, Uuid,
};
use chrono::Utc;
use scylla::CachingSession;

#[partial_model_generator]
#[charybdis_model(
    table_name = flow_steps,
    partition_keys = [node_id, workflow_id],
    clustering_keys = [id],
    secondary_indexes = []
)]
pub struct FlowStep {
    #[serde(rename = "nodeId")]
    pub node_id: Uuid,

    #[serde(rename = "workflowId")]
    pub workflow_id: Uuid,

    #[serde(rename = "flowId")]
    pub flow_id: Uuid,

    #[serde(default = "Uuid::new_v4")]
    pub id: Uuid,

    #[serde(rename = "nodeIds")]
    pub node_ids: Option<List<Uuid>>,

    pub description: Option<Text>,

    #[serde(rename = "descriptionMarkdown")]
    pub description_markdown: Option<Text>,

    #[serde(rename = "inputIdsByNodeId")]
    pub input_ids_by_node_id: Option<Map<Uuid, Frozen<List<Uuid>>>>,

    #[serde(rename = "outputIdsByNodeId")]
    pub output_ids_by_node_id: Option<Map<Uuid, Frozen<List<Uuid>>>>,

    #[serde(rename = "createdAt")]
    pub created_at: Option<Timestamp>,

    #[serde(rename = "updatedAt")]
    pub updated_at: Option<Timestamp>,
}

impl FlowStep {
    pub async fn flow(&self, session: &CachingSession) -> Result<Flow, CharybdisError> {
        let mut flow = Flow::new();
        flow.node_id = self.node_id;
        flow.workflow_id = self.workflow_id;
        flow.id = self.flow_id;

        flow.find_by_primary_key(&session).await
    }

    pub async fn pull_output_id(
        &mut self,
        session: &CachingSession,
        output_id: Uuid,
    ) -> Result<(), CharybdisError> {
        // filter out output_id from output_ids_by_node_id
        let mut output_ids_by_node_id = self.output_ids_by_node_id.clone().unwrap_or_default();

        for (_, output_ids) in output_ids_by_node_id.iter_mut() {
            output_ids.retain(|id| id != &output_id);
        }

        self.output_ids_by_node_id = Some(output_ids_by_node_id);

        self.update_cb(session).await?;

        Ok(())
    }

    pub async fn pull_input_id(
        &mut self,
        session: &CachingSession,
        input_id: Uuid,
    ) -> Result<(), CharybdisError> {
        // filter out input_id from input_ids_by_node_id
        let mut input_ids_by_node_id = self.input_ids_by_node_id.clone().unwrap_or_default();

        for (_, input_ids) in input_ids_by_node_id.iter_mut() {
            input_ids.retain(|id| id != &input_id);
        }

        self.input_ids_by_node_id = Some(input_ids_by_node_id);

        self.update_cb(session).await?;

        Ok(())
    }

    async fn delete_outputs(&self, session: &CachingSession) -> Result<(), CharybdisError> {
        if let Some(output_ids_by_node_id) = &self.output_ids_by_node_id {
            for (_, output_ids) in output_ids_by_node_id.iter() {
                for output_id in output_ids.iter() {
                    let mut output = InputOutput::new();

                    output.node_id = self.node_id;
                    output.workflow_id = self.workflow_id;
                    output.id = output_id.clone();

                    output.find_by_primary_key(session).await?;
                    output.delete_cb(session).await?;
                }
            }
        }

        Ok(())
    }

    async fn delete_outputs_from_non_existent_nodes(
        &mut self,
        session: &CachingSession,
    ) -> Result<(), CharybdisError> {
        let output_ids_by_node_id = self.output_ids_by_node_id.as_mut();
        let node_ids = self.node_ids.as_mut();

        if let Some(output_ids_by_node_id) = output_ids_by_node_id {
            if let Some(node_ids) = node_ids {
                let mut node_ids_to_remove = vec![];

                for (node_id, output_ids) in output_ids_by_node_id.iter() {
                    if !node_ids.contains(node_id) {
                        for output_id in output_ids.iter() {
                            let mut output = InputOutput::new();

                            output.node_id = self.node_id;
                            output.workflow_id = self.workflow_id;
                            output.id = output_id.clone();

                            output.find_by_primary_key(session).await?;
                            output.delete_cb(session).await?;
                        }

                        node_ids_to_remove.push(node_id.clone());
                    }
                }

                for node_id in node_ids_to_remove {
                    output_ids_by_node_id.remove(&node_id);
                }

                self.update_cb(session).await?;
            } else {
                self.output_ids_by_node_id = None;
                self.update_cb(session).await?;
            }
        }

        Ok(())
    }

    async fn disassociate_inputs_from_non_existent_nodes(
        &mut self,
        session: &CachingSession,
    ) -> Result<(), CharybdisError> {
        let input_ids_by_node_id = self.input_ids_by_node_id.as_mut();
        let node_ids = self.node_ids.as_mut();

        if let Some(input_ids_by_node_id) = input_ids_by_node_id {
            if let Some(node_ids) = node_ids {
                let node_ids_to_remove = input_ids_by_node_id
                    .keys()
                    .cloned()
                    .filter(|node_id| !node_ids.contains(node_id))
                    .collect::<Vec<_>>();

                for node_id in node_ids_to_remove {
                    input_ids_by_node_id.remove(&node_id);
                }

                self.update_cb(session).await?;
            } else {
                self.input_ids_by_node_id = None;
                self.update_cb(session).await?;
            }
        }

        Ok(())
    }
}

impl Callbacks for FlowStep {
    created_at_cb_fn!();

    async fn after_insert(&mut self, session: &CachingSession) -> Result<(), CharybdisError> {
        let mut flow = Flow::new();

        flow.node_id = self.node_id;
        flow.workflow_id = self.workflow_id;
        flow.id = self.flow_id;

        flow.append_step(session, self.id).await?;

        Ok(())
    }

    updated_at_cb_fn!();

    async fn after_delete(&mut self, session: &CachingSession) -> Result<(), CharybdisError> {
        let mut flow = Flow::new();

        flow.node_id = self.node_id;
        flow.workflow_id = self.workflow_id;
        flow.id = self.flow_id;

        flow.remove_step(session, self.id).await?;
        self.delete_outputs(session).await?;

        Ok(())
    }
}

partial_flow_step!(
    UpdateFlowStepInputIds,
    node_id,
    workflow_id,
    flow_id,
    id,
    input_ids_by_node_id,
    updated_at
);
impl_updated_at_cb!(UpdateFlowStepInputIds);

partial_flow_step!(
    UpdateFlowStepOutputIds,
    node_id,
    workflow_id,
    flow_id,
    id,
    output_ids_by_node_id,
    updated_at
);
impl_updated_at_cb!(UpdateFlowStepOutputIds);

partial_flow_step!(
    UpdateFlowStepNodeIds,
    node_id,
    workflow_id,
    flow_id,
    id,
    node_ids,
    updated_at
);

impl Callbacks for UpdateFlowStepNodeIds {
    updated_at_cb_fn!();

    async fn after_update(&mut self, session: &CachingSession) -> Result<(), CharybdisError> {
        let mut flow_step = self.as_native().find_by_primary_key(session).await?;

        flow_step
            .delete_outputs_from_non_existent_nodes(session)
            .await?;

        flow_step
            .disassociate_inputs_from_non_existent_nodes(session)
            .await?;

        Ok(())
    }
}

partial_flow_step!(
    FlowStepDescription,
    node_id,
    workflow_id,
    id,
    description,
    description_markdown,
    updated_at
);
sanitize_description_cb!(FlowStepDescription);
