use charybdis::*;

#[partial_model_generator]
#[charybdis_model(
    table_name = "posts",
    partition_keys = ["created_at_day"],
    clustering_keys = ["title"],
    secondary_indexes = []
)]
pub struct Post {
    pub id: Uuid,
    pub created_at_day: Date,
    pub title: Text,
    pub description: Text,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}
