use charybdis::macros::charybdis_model;
use charybdis::types::{Text, Uuid};
use serde::{Deserialize, Serialize};

#[charybdis_model(
    table_name = versioned_descriptions,
    partition_keys = [id],
    clustering_keys = [],
    table_options = r#"
        compression = { 
            'sstable_compression': 'ZstdCompressor',  
            'compression_level': '1', 
            'chunk_length_in_kb': 64
        }
    "#,
)]
#[derive(Serialize, Deserialize, Default)]
pub struct VersionedDescription {
    pub id: Uuid,
    pub description_base64: Option<Text>,
}

// TODO: Use `y-crdt` to extract desc state from only base64 description
impl VersionedDescription {
    pub fn new(description_base64: Option<Text>) -> Self {
        Self {
            id: Uuid::new_v4(),
            description_base64,
        }
    }
}
