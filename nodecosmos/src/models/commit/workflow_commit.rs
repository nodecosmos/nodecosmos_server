use crate::actions::commit_actions::CommitParams;
use crate::models::commit::types::{CommitObjectTypes, Committable};
use crate::models::commit::{Commit, CommitTypes};
use crate::models::workflow::Workflow;
use charybdis::{CharybdisError, InsertWithCallbacks, Map, Text, Uuid};
use scylla::CachingSession;

pub trait WorkflowCommit {
    async fn create_workflow_commit(
        session: &CachingSession,
        params: CommitParams,
        user_id: Uuid,
        workflow: &Workflow,
    ) -> Result<(), CharybdisError>;
}

impl WorkflowCommit for Commit {
    async fn create_workflow_commit(
        session: &CachingSession,
        params: CommitParams,
        user_id: Uuid,
        workflow: &Workflow,
    ) -> Result<(), CharybdisError> {
        let mut commit = Commit::init(
            params,
            workflow.id,
            user_id,
            CommitTypes::Create(CommitObjectTypes::Workflow(Committable::BaseObject)),
        )
        .await?;

        let mut commit_data: Map<Text, Text> = Map::new();

        if let Some(title) = workflow.title.clone() {
            commit_data.insert("title".to_string(), title);
        }

        commit.data = Some(commit_data);

        commit.insert_cb(session).await?;

        Ok(())
    }
}
