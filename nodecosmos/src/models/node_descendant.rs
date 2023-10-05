use charybdis::{Double, Text, Uuid};
use charybdis_macros::{charybdis_model, partial_model_generator};

#[derive(Clone)]
#[partial_model_generator]
#[charybdis_model(
    table_name = node_descendants,
    partition_keys = [root_id],
    clustering_keys = [order_index, id],
    secondary_indexes = [],
)]
pub struct NodeDescendant {
    #[serde(rename = "rootId")]
    pub root_id: Uuid,

    pub id: Uuid,

    #[serde(rename = "parentId")]
    pub parent_id: Option<Uuid>,

    pub title: Option<Text>,

    #[serde(rename = "order")]
    pub order_index: Option<Double>,
}

partial_node_descendant!(UpdateNodeDescendantTitle, root_id, order_index, id, title);
partial_node_descendant!(DeleteNodeDescendant, root_id, order_index, id);
