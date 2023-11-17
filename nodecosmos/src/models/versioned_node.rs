pub mod create;

use crate::constants::MAX_PARALLEL_REQUESTS;
use crate::errors::NodecosmosError;
use crate::models::versioned_node_ancestors::VersionedNodeAncestors;
use charybdis::errors::CharybdisError;
use charybdis::macros::charybdis_model;
use charybdis::operations::Find;
use charybdis::types::{Double, Text, Timestamp, Uuid};
use scylla::CachingSession;
use serde::{Deserialize, Serialize};
use std::future::Future;

#[charybdis_model(
    table_name = versioned_nodes,
    partition_keys = [node_id],
    clustering_keys = [created_at, id],
    table_options = r#"
        CLUSTERING ORDER BY (created_at DESC)
        AND compression = {'sstable_compression': 'DeflateCompressor', 'chunk_length_in_kb': 4};
    "#
)]
#[derive(Serialize, Deserialize, Default, Clone)]
pub struct VersionedNode {
    #[serde(rename = "nodeId")]
    pub node_id: Uuid,

    pub id: Uuid,
    pub title: Text,

    #[serde(rename = "parentId")]
    pub parent_id: Option<Uuid>,

    #[serde(rename = "versionedDescriptionId")]
    pub versioned_description_id: Option<Uuid>,

    #[serde(rename = "versionedAncestorsId")]
    pub versioned_ancestors_id: Uuid,

    #[serde(rename = "versionedNodeDescendantsId")]
    pub versioned_descendants_id: Option<Uuid>,

    #[serde(rename = "versionedWorkflowId")]
    pub versioned_workflow_id: Option<Uuid>,

    #[serde(rename = "orderIndex")]
    pub order_index: Double,

    #[serde(rename = "createdAt")]
    pub created_at: Option<Timestamp>,

    #[serde(rename = "updatedAt")]
    pub updated_at: Option<Timestamp>,

    #[serde(rename = "userId")]
    pub user_id: Option<Uuid>,
}

impl VersionedNode {
    fn find_first_by_node_id<'a>(
        session: &'a CachingSession,
        node_id: &'a Uuid,
    ) -> impl Future<Output = Result<Self, CharybdisError>> + 'a {
        let res = find_first_versioned_node!(session, "node_id = ? LIMIT 1", (node_id,));

        res
    }

    pub async fn init_from(v_node: &Self) -> Result<Self, NodecosmosError> {
        let mut node_version = v_node.clone();
        node_version.id = Uuid::new_v4();

        Ok(node_version)
    }

    pub async fn init_from_latest(session: &CachingSession, node_id: &Uuid) -> Result<Self, NodecosmosError> {
        let mut node_version = VersionedNode::find_first_by_node_id(session, node_id).await?;
        node_version.id = Uuid::new_v4();

        Ok(node_version)
    }

    pub async fn versioned_ancestors(
        &self,
        session: &CachingSession,
    ) -> Result<VersionedNodeAncestors, NodecosmosError> {
        let res = VersionedNodeAncestors::find_by_primary_key_value(session, (self.versioned_ancestors_id,)).await?;

        Ok(res)
    }

    pub async fn latest_ancestors(&self, session: &CachingSession) -> Result<Vec<VersionedNode>, NodecosmosError> {
        let va = self.versioned_ancestors(session).await?;
        let ancestor_ids = va.ancestor_ids;

        if let Some(ancestor_ids) = ancestor_ids {
            let mut versioned_nodes = vec![];

            let ancestor_ids_chunks = ancestor_ids.chunks(MAX_PARALLEL_REQUESTS);
            for ancestor_ids_chunk in ancestor_ids_chunks {
                let mut futures = vec![];

                for ancestor_id in ancestor_ids_chunk {
                    let future = VersionedNode::find_first_by_node_id(session, ancestor_id);
                    futures.push(future);
                }

                let futures_res = futures::future::join_all(futures).await;

                for res in futures_res {
                    let versioned_node = res?;
                    versioned_nodes.push(versioned_node);
                }
            }

            return Ok(versioned_nodes);
        }

        Ok(vec![])
    }
}

pub trait VersionedNodePluckable {
    fn pluck_versioned_descendants_id(&self) -> Vec<Uuid>;
}
impl VersionedNodePluckable for Vec<VersionedNode> {
    fn pluck_versioned_descendants_id(&self) -> Vec<Uuid> {
        self.iter().filter_map(|item| item.versioned_descendants_id).collect()
    }
}
