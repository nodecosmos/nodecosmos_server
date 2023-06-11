use crate::models::flow::Flow;
use crate::models::helpers::{
    created_at_cb_fn, impl_empty_cb, impl_updated_at_cb, updated_at_cb_fn,
};
use charybdis::{
    charybdis_model, partial_model_generator, Callbacks, CharybdisError, Find, Frozen, List, Map,
    New, Timestamp, UpdateWithCallbacks, Uuid,
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
    async fn next_flow_step(
        &self,
        session: &CachingSession,
    ) -> Result<Option<UpdateFlowStepOutputIds>, CharybdisError> {
        let mut flow = Flow::new();

        flow.node_id = self.node_id;
        flow.workflow_id = self.workflow_id;
        flow.id = self.flow_id;

        flow = flow.find_by_primary_key(session).await?;

        let next_flow_step_id = match flow.step_ids {
            Some(ref step_ids) => step_ids.iter().skip_while(|&uuid| uuid == &self.id).next(),
            None => None,
        };

        match next_flow_step_id {
            Some(next_flow_step_id) => {
                let mut next_flow_step = UpdateFlowStepOutputIds::new();

                next_flow_step.node_id = self.node_id;
                next_flow_step.workflow_id = self.workflow_id;
                next_flow_step.flow_id = self.flow_id;
                next_flow_step.id = next_flow_step_id.clone();

                next_flow_step.find_by_primary_key(session).await?;

                Ok(Some(next_flow_step))
            }
            None => Ok(None),
        }
    }

    async fn dissociate_outputs_from_next_flow_step(
        &mut self,
        session: &CachingSession,
    ) -> Result<(), CharybdisError> {
        let next_flow_step = self.next_flow_step(session).await?;

        if let Some(mut next_flow_step) = next_flow_step {
            match next_flow_step.output_ids_by_node_id {
                Some(ref mut output_ids_by_node_id) => {
                    for (_, output_ids) in output_ids_by_node_id.iter_mut() {
                        if let Some(pos) = output_ids.iter().position(|&id| id == self.id) {
                            output_ids.remove(pos);
                        }
                    }

                    next_flow_step.update_cb(session).await?;
                }

                None => {}
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
        self.dissociate_outputs_from_next_flow_step(session).await?;

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

partial_flow_step!(DeleteFlowStep, node_id, workflow_id, flow_id, id);
impl_empty_cb!(DeleteFlowStep);

partial_flow_step!(
    UpdateFlowStepNodeIds,
    node_id,
    workflow_id,
    flow_id,
    id,
    node_ids,
    updated_at
);

impl_updated_at_cb!(UpdateFlowStepNodeIds);
