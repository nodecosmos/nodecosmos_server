use charybdis::{Set, Uuid};
use charybdis_macros::charybdis_view_model;

#[charybdis_view_model(
    table_name=auth_node_by_id,
    base_table=nodes,
    partition_keys=[id],
    clustering_keys=[root_id]
)]
pub struct AuthNodeById {
    pub root_id: Uuid,
    pub id: Uuid,
    pub editor_ids: Option<Set<Uuid>>,
    pub owner_id: Option<Uuid>,
    pub is_public: Option<bool>,
}
