use charybdis::Uuid;
use charybdis_macros::charybdis_view_model;

#[charybdis_view_model(
    table_name=child_ids_by_parent_id,
    base_table=nodes,
    partition_keys=[id],
    clustering_keys=[parent_id]
)]
pub struct ChildIdsByParentId {
    #[serde(rename = "parentId")]
    pub parent_id: Uuid,

    pub id: Option<Uuid>,
}
