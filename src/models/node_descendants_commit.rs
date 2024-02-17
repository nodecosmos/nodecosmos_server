use crate::errors::NodecosmosError;
use charybdis::macros::charybdis_model;
use charybdis::types::{Frozen, Map, Uuid};
use futures::StreamExt;
use scylla::CachingSession;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[charybdis_model(
    table_name = node_descendants_commits,
    partition_keys = [id],
    clustering_keys = [],
    table_options = r#"
        compression = { 
            'sstable_compression': 'DeflateCompressor',
            'chunk_length_in_kb': '64kb'
        }
    "#
)]
#[derive(Serialize, Deserialize, Default)]
pub struct NodeDescendantsCommit {
    pub id: Uuid,
    pub node_id: Uuid,
    pub descendant_node_commit_id_by_id: Frozen<Map<Uuid, Uuid>>,
}

impl NodeDescendantsCommit {
    pub fn new(node_id: Uuid, descendant_node_commit_id_by_id: HashMap<Uuid, Uuid>) -> Self {
        Self {
            id: Uuid::new_v4(),
            node_id,
            descendant_node_commit_id_by_id,
        }
    }

    pub async fn find_by_ids(session: &CachingSession, ids: Vec<Uuid>) -> Result<Vec<Self>, NodecosmosError> {
        let mut res = vec![];

        let mut commits = find_node_descendants_commit!("id IN ?", (ids,))
            .execute(session)
            .await?;

        while let Some(commit) = commits.next().await {
            res.push(commit?);
        }

        Ok(res)
    }

    pub async fn find_grouped_by_node_id(
        session: &CachingSession,
        ids: Vec<Uuid>,
    ) -> Result<HashMap<Uuid, Self>, NodecosmosError> {
        let commits = Self::find_by_ids(session, ids).await?;
        let mut res = HashMap::new();

        for commit in commits {
            res.insert(commit.node_id, commit);
        }

        Ok(res)
    }
}
