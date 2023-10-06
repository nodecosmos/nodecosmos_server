pub(crate) mod node_commit;
pub(crate) mod types;
pub(crate) mod workflow_commit;

use crate::actions::commit_actions::CommitParams;
use crate::models::commit::types::CommitTypes;
use crate::models::helpers::impl_default_callbacks;
use charybdis::{CharybdisError, Delete, InsertWithCallbacks, Map, New, Text, Timestamp, Uuid};
use charybdis_macros::{charybdis_model, partial_model_generator};
use scylla::CachingSession;

#[partial_model_generator]
#[charybdis_model(
    table_name = commits,
    partition_keys = [contribution_request_id],
    clustering_keys = [created_at, id],
    secondary_indexes = [],
    table_options = "WITH CLUSTERING ORDER BY (created_at DESC)"
)]
pub struct Commit {
    #[serde(rename = "nodeId")]
    pub node_id: Uuid,

    #[serde(rename = "contributionRequestId")]
    pub contribution_request_id: Uuid,

    pub id: Uuid,

    #[serde(rename = "objectId")]
    pub object_id: Uuid,

    #[serde(rename = "commitType")]
    pub commit_type: Text,

    #[serde(rename = "userId")]
    pub user_id: Uuid,

    pub data: Option<Map<Text, Text>>,

    #[serde(rename = "createdAt")]
    pub created_at: Option<Timestamp>,

    #[serde(rename = "updatedAt")]
    pub updated_at: Option<Timestamp>,
}

impl Commit {
    async fn init(
        params: CommitParams,
        object_id: Uuid,
        user_id: Uuid,
        commit_type: CommitTypes,
    ) -> Result<Commit, CharybdisError> {
        let mut commit = Commit::new();

        commit.node_id = params.node_id;
        commit.contribution_request_id = params.contribution_request_id;

        commit.object_id = object_id;
        commit.user_id = user_id;
        commit.commit_type = commit_type.to_string();

        Ok(commit)
    }

    pub async fn create_update_object_commit(
        session: &CachingSession,
        params: CommitParams,
        user_id: Uuid,
        object_id: Uuid,
        attribute: &str,
        value: Text,
        commit_type: CommitTypes,
    ) -> Result<(), CharybdisError> {
        let mut commit = Commit::init(params, object_id, user_id, commit_type).await?;
        let mut commit_data: Map<Text, Text> = Map::new();

        commit_data.insert(attribute.to_string(), value);
        commit.data = Some(commit_data);

        commit.insert_cb(session).await?;

        Ok(())
    }

    pub async fn create_delete_object_commit(
        session: &CachingSession,
        params: CommitParams,
        user_id: Uuid,
        object_id: Uuid,
        commit_type: CommitTypes,
    ) -> Result<(), CharybdisError> {
        let mut commit = Commit::init(params, object_id, user_id, commit_type).await?;

        commit.insert_cb(session).await?;

        Ok(())
    }

    pub async fn delete_contribution_request_commits(
        session: &CachingSession,
        contribution_request_id: Uuid,
    ) -> Result<(), CharybdisError> {
        let mut commit = Commit::new();
        commit.contribution_request_id = contribution_request_id;
        commit.delete_by_partition_key(session).await?;

        Ok(())
    }
}

impl_default_callbacks!(Commit);
