// use crate::errors::NodecosmosError;
// use crate::models::utils::impl_default_callbacks;
// use charybdis::macros::charybdis_model;
// use charybdis::types::{SmallInt, Timestamp, Uuid};
// use serde::{Deserialize, Serialize};
//
// #[charybdis_model(
//     table_name = task_sections,
//     partition_keys = [branch_id],
//     clustering_keys = [node_id, id],
// )]
// #[derive(Serialize, Deserialize, Default, Clone)]
// #[serde(rename_all = "camelCase")]
// pub struct TaskSection {
//     pub branch_id: Uuid,
//     pub node_id: Uuid,
//     pub id: Uuid,
//     pub order_index: SmallInt,
//     pub created_at: Timestamp,
//     pub updated_at: Timestamp,
// }
//
// impl_default_callbacks!(TaskSection);
