pub mod create;
pub mod reorder;

use crate::constants::MAX_PARALLEL_REQUESTS;
use crate::errors::NodecosmosError;
use crate::models::node_descendants_commit::NodeDescendantsCommit;
use crate::models::node_tree_position_commit::NodeTreePositionCommit;
use charybdis::errors::CharybdisError;
use charybdis::macros::charybdis_model;
use charybdis::types::{Text, Timestamp, Uuid};
use chrono::Utc;
use scylla::CachingSession;
use serde::{Deserialize, Serialize};

#[charybdis_model(
    table_name = node_commits,
    partition_keys = [node_id, branch_id],
    clustering_keys = [created_at, id],
    table_options = r#"
        CLUSTERING ORDER BY (created_at DESC) AND
        COMPRESSION = { 
            'sstable_compression': 'DeflateCompressor',
            'chunk_length_in_kb': 64
        }
    "#
)]
#[derive(Serialize, Deserialize, Default, Clone)]
pub struct NodeCommit {
    #[serde(default, rename = "nodeId")]
    pub node_id: Uuid,

    #[serde(rename = "branchId")]
    pub branch_id: Uuid,

    #[serde(rename = "createdAt")]
    pub created_at: Timestamp,

    pub id: Uuid,

    #[serde(rename = "prevVersionId")]
    pub prev_commit_id: Option<Uuid>,

    #[serde(rename = "prevVersionBranchId")]
    pub prev_commit_branch_id: Option<Uuid>,

    #[serde(rename = "userId")]
    pub user_id: Option<Uuid>,

    #[serde(rename = "nodeTitle")]
    pub node_title: Text,

    #[serde(rename = "nodeCreatorId")]
    pub node_creator_id: Option<Uuid>,

    #[serde(rename = "nodeCreatedAt")]
    pub node_created_at: Option<Timestamp>,

    #[serde(rename = "descriptionCommitId")]
    pub description_commit_id: Option<Uuid>,

    #[serde(rename = "nodeTreePositionCommitId")]
    pub node_tree_position_commit_id: Uuid,

    #[serde(rename = "nodeDescendantsCommitId")]
    pub node_descendants_commit_id: Option<Uuid>,

    #[serde(rename = "workflowCommitId")]
    pub workflow_commit_id: Option<Uuid>,
}

impl NodeCommit {
    pub fn init_from(commit: &Self, branch_id: Uuid, user_id: Option<Uuid>) -> Self {
        let mut commit = commit.clone();
        commit.id = Uuid::new_v4();
        commit.created_at = Utc::now();
        commit.branch_id = branch_id;
        commit.user_id = user_id;
        commit.prev_commit_id = Some(commit.id);
        commit.prev_commit_branch_id = Some(commit.branch_id);

        commit
    }

    pub async fn init_from_latest(
        session: &CachingSession,
        node_id: Uuid,
        branch_id: Uuid,
        user_id: Uuid,
    ) -> Result<Self, NodecosmosError> {
        let mut commit = NodeCommit::find_latest(session, &node_id, &branch_id).await?;

        commit.id = Uuid::new_v4();
        commit.created_at = Utc::now();
        commit.branch_id = branch_id;
        commit.user_id = Some(user_id);
        commit.prev_commit_id = Some(commit.id);
        commit.prev_commit_branch_id = Some(commit.branch_id);

        Ok(commit)
    }

    async fn find_latest(session: &CachingSession, node_id: &Uuid, branch_id: &Uuid) -> Result<Self, CharybdisError> {
        let res = find_first_node_commit!(
            "node_id = ? AND branch_id in ? LIMIT 1",
            (node_id, vec![branch_id, node_id])
        )
        .execute(session)
        .await?;

        Ok(res)
    }

    async fn latest_by_node_ids(
        session: &CachingSession,
        branch_id: &Uuid,
        ids: &Vec<Uuid>,
    ) -> Result<Vec<Self>, NodecosmosError> {
        let mut commits = vec![];

        let ids_chunks = ids.chunks(MAX_PARALLEL_REQUESTS);
        for ids_chunk in ids_chunks {
            let mut futures = vec![];

            for ancestor_id in ids_chunk {
                let future = NodeCommit::find_latest(session, ancestor_id, branch_id);
                futures.push(future);
            }

            let futures_res = futures::future::join_all(futures).await;

            for commit in futures_res {
                commits.push(commit?);
            }
        }

        return Ok(commits);
    }

    pub async fn prev_commit(&self, session: &CachingSession) -> Result<Option<Self>, CharybdisError> {
        if let (Some(prev_commit_id), Some(prev_commit_branch_id)) = (self.prev_commit_id, self.prev_commit_branch_id) {
            let res = find_first_node_commit!(
                "node_id = ? AND branch_id = ? AND created_at =< ? AND id = ? LIMIT 1",
                (self.node_id, prev_commit_branch_id, self.created_at, prev_commit_id)
            )
            .execute(session)
            .await?;

            return Ok(Some(res));
        }

        Ok(None)
    }

    pub async fn tree_position_commit(
        &mut self,
        session: &CachingSession,
    ) -> Result<NodeTreePositionCommit, NodecosmosError> {
        let res = NodeTreePositionCommit::find_by_id(self.node_tree_position_commit_id)
            .execute(session)
            .await?;

        Ok(res)
    }

    pub async fn node_descendants_commit(
        &self,
        session: &CachingSession,
    ) -> Result<Option<NodeDescendantsCommit>, NodecosmosError> {
        if let Some(node_descendants_commit_id) = self.node_descendants_commit_id {
            let res = NodeDescendantsCommit::find_by_id(node_descendants_commit_id)
                .execute(session)
                .await?;

            return Ok(Some(res));
        }

        Ok(None)
    }

    pub async fn latest_ancestor_commits(&mut self, session: &CachingSession) -> Result<Vec<Self>, NodecosmosError> {
        let vtp = self.tree_position_commit(session).await?;

        if let Some(ancestor_ids) = &vtp.ancestor_ids {
            let ancestor_ids = ancestor_ids.iter().cloned().collect();
            return Self::latest_by_node_ids(session, &self.branch_id, &ancestor_ids).await;
        }

        Ok(vec![])
    }

    pub async fn latest_descendant_commits(&self, session: &CachingSession) -> Result<Vec<Self>, NodecosmosError> {
        let vd = self.node_descendants_commit(session).await?;

        if let Some(vd) = vd {
            let descendant_ids = vd.descendant_node_commit_id_by_id.keys().cloned().collect();

            return Self::latest_by_node_ids(session, &self.branch_id, &descendant_ids).await;
        }

        Ok(vec![])
    }
}
