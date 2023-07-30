use crate::actions::commit_actions::CommitParams;
use crate::models::commit::types::{Committable, ObjectTypes};
use crate::models::commit::{Commit, CommitTypes};
use crate::models::node::Node;
use charybdis::{CharybdisError, InsertWithCallbacks, Map, Text, Uuid};
use scylla::CachingSession;

pub trait NodeCommit {
    async fn create_node_commit(
        session: &CachingSession,
        params: CommitParams,
        user_id: Uuid,
        node: &Node,
    ) -> Result<(), CharybdisError>;
}

impl NodeCommit for Commit {
    async fn create_node_commit(
        session: &CachingSession,
        params: CommitParams,
        user_id: Uuid,
        node: &Node,
    ) -> Result<(), CharybdisError> {
        let mut commit = Commit::init(
            params,
            node.id,
            user_id,
            CommitTypes::Create(ObjectTypes::Node(Committable::BaseObject)),
        )
        .await?;

        let mut commit_data: Map<Text, Text> = Map::new();

        if let Some(title) = node.title.clone() {
            commit_data.insert("title".to_string(), title);
        }
        if let Some(parent_id) = node.parent_id {
            commit_data.insert("parent_id".to_string(), parent_id.to_string());
        }
        commit_data.insert("root_id".to_string(), node.root_id.to_string());
        commit.data = Some(commit_data);

        commit.insert_cb(session).await?;

        Ok(())
    }
}
