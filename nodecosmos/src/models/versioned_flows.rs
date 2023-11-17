use charybdis::macros::charybdis_model;
use charybdis::types::{Map, Uuid};
use serde::{Deserialize, Serialize};

#[charybdis_model(
    table_name = versioned_flows,
    partition_keys = [id],
    clustering_keys = [],
    table_options = r#"
        compression = {'sstable_compression': 'DeflateCompressor', 'chunk_length_in_kb': 4};
    "#,
)]
#[derive(Serialize, Deserialize, Default)]
pub struct VersionedFlows {
    pub id: Uuid,
    pub flow_version_by_id: Option<Map<Uuid, Uuid>>,
}
