use crate::models::helpers::impl_default_callbacks;
use charybdis::*;
use chrono::Utc;

#[partial_model_generator]
#[charybdis_model(
    table_name = "input_outputs",
    partition_keys = ["id"],
    clustering_keys = [],
    secondary_indexes = ["title"]
)]
pub struct InputOutput {
    pub id: Uuid,
    pub title: Text,
    #[serde(rename = "dataType")]
    pub data_type: Text,
    pub value: Text,
    #[serde(rename = "createdAt")]
    pub created_at: Option<Timestamp>,
    #[serde(rename = "updatedAt")]
    pub updated_at: Option<Timestamp>,
}
impl_default_callbacks!(WorkflowStep);
