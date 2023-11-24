use charybdis::macros::charybdis_model;
use charybdis::types::{Text, Uuid};
use serde::{Deserialize, Serialize};

#[charybdis_model(
    table_name = versioned_descriptions,
    partition_keys = [id],
    clustering_keys = [],
    table_options = r#"
        compression = { 
            'sstable_compression': 'DeflateCompressor',  
        }
    "#,
)]
#[derive(Serialize, Deserialize, Default)]
pub struct DescriptionCommit {
    pub id: Uuid,
    pub description_base64: Option<Text>,
}

// TODO: Use `y-crdt` to extract desc state from only base64 description
impl DescriptionCommit {
    pub fn new(description_base64: Option<Text>) -> Self {
        Self {
            id: Uuid::new_v4(),
            description_base64,
        }
    }
}
