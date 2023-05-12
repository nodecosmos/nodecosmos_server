use crate::models::helpers::{impl_default_callbacks, impl_updated_at_cb};
use charybdis::{charybdis_model, execute, partial_model_generator, List, Text, Timestamp, Uuid};
use chrono::Utc;
use scylla::CachingSession;

#[partial_model_generator]
#[charybdis_model(
    table_name = "flows",
    partition_keys = ["node_id", "workflow_id"],
    clustering_keys = ["id"],
    secondary_indexes = []
)]
pub struct Flow {
    #[serde(rename = "nodeId")]
    pub node_id: Uuid,

    #[serde(rename = "id")]
    pub id: Uuid,

    #[serde(rename = "workflowId")]
    pub workflow_id: Uuid,

    pub title: Text,
    pub description: Text,

    #[serde(rename = "createdAt")]
    pub created_at: Option<Timestamp>,

    #[serde(rename = "updatedAt")]
    pub updated_at: Option<Timestamp>,

    #[serde(rename = "stepIds")]
    pub step_ids: List<Uuid>,
}

impl Flow {
    pub async fn append_step(
        &mut self,
        session: &CachingSession,
        step_id: Uuid,
    ) -> Result<(), charybdis::CharybdisError> {
        execute(
            session,
            Flow::PUSH_TO_STEP_IDS_QUERY,
            (step_id, self.node_id, self.workflow_id, self.id),
        )
        .await?;

        Ok(())
    }

    pub async fn remove_step(
        &mut self,
        session: &CachingSession,
        step_id: Uuid,
    ) -> Result<(), charybdis::CharybdisError> {
        execute(
            session,
            Flow::PULL_FROM_STEP_IDS_QUERY,
            (step_id, self.node_id, self.workflow_id, self.id),
        )
        .await?;

        Ok(())
    }
}

impl_default_callbacks!(Flow);

partial_flow!(UpdateFlowTitle, node_id, workflow_id, id, title, updated_at);
impl_updated_at_cb!(UpdateFlowTitle);

partial_flow!(
    UpdateFlowDescription,
    node_id,
    workflow_id,
    id,
    description,
    updated_at
);
impl_updated_at_cb!(UpdateFlowDescription);
