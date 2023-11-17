use charybdis::macros::charybdis_model;
use charybdis::types::{Text, Uuid};
use serde::{Deserialize, Serialize};

#[charybdis_model(
    table_name = versioned_descriptions,
    partition_keys = [id],
    clustering_keys = [],
    table_options = r#"
        compression = {'sstable_compression': 'DeflateCompressor', 'chunk_length_in_kb': 4};
    "#,
)]
#[derive(Serialize, Deserialize, Default)]
pub struct VersionedDescription {
    pub id: Uuid,
    pub description: Option<Text>,
    pub short_description: Option<Text>,
    pub description_markdown: Option<Text>,
    pub description_base64: Option<Text>,
}

// TODO: Use `y-crdt` to extract state from only base64 description, so we can remove other fields for space efficiency
impl VersionedDescription {
    pub fn new(
        description: Option<Text>,
        short_description: Option<Text>,
        description_markdown: Option<Text>,
        description_base64: Option<Text>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            description,
            short_description,
            description_markdown,
            description_base64,
        }
    }
}
