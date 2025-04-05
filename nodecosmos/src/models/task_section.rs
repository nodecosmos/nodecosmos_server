use crate::errors::NodecosmosError;
use crate::models::utils::{impl_default_callbacks, impl_updated_at_cb};
use charybdis::macros::charybdis_model;
use charybdis::types::{Double, Text, Timestamp, Uuid};
use serde::{Deserialize, Serialize};

#[charybdis_model(
    table_name = task_sections,
    partition_keys = [branch_id],
    clustering_keys = [node_id, id],
)]
#[derive(Serialize, Deserialize, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TaskSection {
    pub title: Text,
    pub branch_id: Uuid,
    pub node_id: Uuid,

    #[serde(default)]
    pub id: Uuid,

    pub order_index: Double,

    #[serde(default = "chrono::Utc::now")]
    pub created_at: Timestamp,

    #[serde(default = "chrono::Utc::now")]
    pub updated_at: Timestamp,
}

impl_default_callbacks!(TaskSection);

partial_task_section!(
    UpdateOrderIndexTaskSection,
    branch_id,
    node_id,
    id,
    order_index,
    updated_at
);

impl_updated_at_cb!(UpdateOrderIndexTaskSection);

partial_task_section!(UpdateTitleTaskSection, branch_id, node_id, id, title, updated_at);

impl_updated_at_cb!(UpdateTitleTaskSection);
