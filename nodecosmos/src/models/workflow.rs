use crate::models::helpers::impl_default_callbacks;
use charybdis::*;
use chrono::Utc;

#[partial_model_generator]
#[charybdis_model(
    table_name = "workflows",
    partition_keys = ["node_id"],
    clustering_keys = [],
    secondary_indexes = []
)]
pub struct Workflow {
    #[serde(rename = "nodeId")]
    pub node_id: Uuid,
    pub id: Uuid,
    pub title: Text,
    pub description: Text,

    #[serde(rename = "createdAt")]
    pub created_at: Option<Timestamp>,

    #[serde(rename = "updatedAt")]
    pub updated_at: Option<Timestamp>,
}
impl_default_callbacks!(Workflow);
