use charybdis::macros::charybdis_model;
use charybdis::stream::CharybdisModelStream;
use charybdis::types::{Double, Text, Uuid};
use scylla::CachingSession;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use crate::errors::NodecosmosError;
use nodecosmos_macros::{Id, RootId};

#[charybdis_model(
    table_name = node_descendants,
    partition_keys = [root_id],
    clustering_keys = [branch_id, node_id, order_index, id],
    table_options = r#"
        gc_grace_seconds = 432000 AND
        compression = {
            'sstable_compression': 'LZ4Compressor',
            'chunk_length_in_kb': '64kb'
        }
    "#,
)]
#[derive(Id, RootId, Serialize, Deserialize, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct NodeDescendant {
    /// Tree root node id.
    pub root_id: Uuid,

    pub branch_id: Uuid,

    /// Current root id.
    pub node_id: Uuid,

    pub order_index: Double,
    pub id: Uuid,
    pub parent_id: Uuid,
    pub title: Text,
}

impl NodeDescendant {
    pub async fn find_by_node_ids(
        db_session: &CachingSession,
        root_id: Uuid,
        node_ids: &HashSet<Uuid>,
    ) -> Result<CharybdisModelStream<NodeDescendant>, NodecosmosError> {
        find_node_descendant!(
            "root_id = ? AND branch_id IN ? AND node_id IN ?",
            (root_id, node_ids, node_ids,)
        )
        .execute(db_session)
        .await
        .map_err(NodecosmosError::from)
    }

    pub async fn find_by_branch_id_and_node_ids(
        db_session: &CachingSession,
        root_id: Uuid,
        branch_id: Uuid,
        node_ids: &HashSet<Uuid>,
    ) -> Result<CharybdisModelStream<NodeDescendant>, NodecosmosError> {
        find_node_descendant!(
            "root_id = ? AND branch_id = ? AND node_id IN ?",
            (root_id, branch_id, node_ids,)
        )
        .execute(db_session)
        .await
        .map_err(NodecosmosError::from)
    }
}
