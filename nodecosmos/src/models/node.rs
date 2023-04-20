use charybdis::prelude::*;

#[partial_model_generator]
#[charybdis_model(table_name = "nodes",
                  partition_keys = ["root_id"],
                  clustering_keys = [id],
                  secondary_indexes = [])]
pub struct Node {
    pub id: Uuid,
    pub root_id: Uuid,
    pub parent_id: Option<Uuid>,
    pub title: Text,
    pub description: Text,
    pub descendant_ids: Option<List<Uuid>>,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}
