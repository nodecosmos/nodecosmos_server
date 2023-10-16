use crate::actions::commit_actions::CommitParams;
use crate::errors::NodecosmosError;
use crate::models::commit::types::{CommitObjectTypes, Committable};
use crate::models::commit::{Commit, CommitTypes};
use crate::models::node::Node;
use charybdis::operations::InsertWithCallbacks;
use charybdis::types::{Map, Text, Uuid};
use scylla::CachingSession;

pub trait NodeCommit {
    async fn create_node_commit(
        session: &CachingSession,
        params: CommitParams,
        user_id: Uuid,
        node: &Node,
    ) -> Result<(), NodecosmosError>;
}

impl NodeCommit for Commit {
    async fn create_node_commit(
        session: &CachingSession,
        params: CommitParams,
        user_id: Uuid,
        node: &Node,
    ) -> Result<(), NodecosmosError> {
        let mut commit = Commit::init(
            params,
            node.id,
            user_id,
            CommitTypes::Create(CommitObjectTypes::Node(Committable::BaseObject)),
        )
        .await?;

        let mut commit_data: Map<Text, Text> = Map::new();

        commit_data.insert("title".to_string(), node.title.clone());

        if let Some(parent_id) = node.parent_id {
            commit_data.insert("parent_id".to_string(), parent_id.to_string());
        }
        commit_data.insert("root_id".to_string(), node.root_id.to_string());
        commit.data = Some(commit_data);

        commit.insert_cb(session).await?;

        Ok(())
    }
}
