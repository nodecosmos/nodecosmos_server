use crate::models::helpers::impl_default_callbacks;
use charybdis::*;
use chrono::Utc;

#[partial_model_generator]
#[charybdis_model(
    table_name = "workflow_steps",
    partition_keys = ["workflow_id"],
    clustering_keys = ["id"],
    secondary_indexes = []
)]
pub struct WorkflowStep {
    #[serde(rename = "workflowId")]
    pub workflow_id: Uuid,
    pub id: Uuid,
    pub title: Text,
    pub description: Text,
    #[serde(rename = "inputIds")]
    pub input_ids: Option<Set<Uuid>>,
    #[serde(rename = "outputIds")]
    pub output_ids: Option<Set<Uuid>>,
    #[serde(rename = "createdAt")]
    pub created_at: Option<Timestamp>,
    #[serde(rename = "updatedAt")]
    pub updated_at: Option<Timestamp>,
}
impl_default_callbacks!(WorkflowStep);
