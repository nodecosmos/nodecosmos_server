use charybdis::macros::charybdis_model;
use charybdis::types::{Text, Uuid};
use serde::{Deserialize, Serialize};

#[charybdis_model(
    table_name = versioned_descriptions,
    partition_keys = [version],
    clustering_keys = [],
    table_options = r#"
        compression = {'sstable_compression': 'DEFLATE'};
    "#,
)]
#[derive(Serialize, Deserialize, Default)]
pub struct VersionedDescription {
    pub version: Uuid,
    pub description: Text,
    pub short_description: Option<Text>,
    pub description_markdown: Text,
    pub description_base64: Text,
}

impl VersionedDescription {
    pub fn new(
        description: Text,
        short_description: Option<Text>,
        description_markdown: Text,
        description_base64: Text,
    ) -> Self {
        Self {
            version: Uuid::new_v4(),
            description,
            short_description,
            description_markdown,
            description_base64,
        }
    }
}
