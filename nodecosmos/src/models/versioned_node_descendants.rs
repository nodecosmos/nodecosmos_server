use crate::errors::NodecosmosError;
use charybdis::macros::charybdis_model;
use charybdis::types::{Map, Uuid};
use futures::StreamExt;
use scylla::CachingSession;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[charybdis_model(
    table_name = versioned_node_descendants,
    partition_keys = [id],
    clustering_keys = [],
    table_options = r#"
        compression = {'sstable_compression': 'DeflateCompressor', 'chunk_length_in_kb': 4};
    "#,
)]
#[derive(Serialize, Deserialize, Default)]
pub struct VersionedNodeDescendants {
    pub id: Uuid,
    pub node_id: Uuid,
    pub descendant_version_by_id: Map<Uuid, Uuid>,
}

impl VersionedNodeDescendants {
    pub fn new(node_id: Uuid, descendant_version_by_id: HashMap<Uuid, Uuid>) -> Self {
        Self {
            id: Uuid::new_v4(),
            node_id,
            descendant_version_by_id,
        }
    }

    pub async fn find_by_ids(session: &CachingSession, ids: Vec<Uuid>) -> Result<Vec<Self>, NodecosmosError> {
        let mut res = vec![];

        let mut v_node_descendants = find_versioned_node_descendants!(session, "id IN ?", (ids,)).await?;

        while let Some(ver_desc) = v_node_descendants.next().await {
            res.push(ver_desc?);
        }

        Ok(res)
    }

    pub async fn find_grouped_by_node_id(
        session: &CachingSession,
        ids: Vec<Uuid>,
    ) -> Result<HashMap<Uuid, Self>, NodecosmosError> {
        let v_node_descendants = Self::find_by_ids(session, ids).await?;
        let mut res = HashMap::new();

        for ver_desc in v_node_descendants {
            res.insert(ver_desc.node_id, ver_desc);
        }

        Ok(res)
    }
}
