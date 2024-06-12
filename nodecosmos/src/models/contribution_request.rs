use charybdis::batch::CharybdisBatch;
use std::collections::HashSet;
use std::fmt;

use charybdis::callbacks::Callbacks;
use charybdis::macros::charybdis_model;
use charybdis::operations::Delete;
use charybdis::types::{Frozen, Set, Text, Timestamp, Uuid};
use scylla::CachingSession;
use serde::{Deserialize, Serialize};

use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::branch::Branch;
use crate::models::description::Description;
use crate::models::flow::PkFlow;
use crate::models::flow_step::PkFlowStep;
use crate::models::io::DeleteIo;
use crate::models::node::{Node, PkNode};
use crate::models::notification::{Notification, NotificationType};
use crate::models::traits::Branchable;
use crate::models::udts::Profile;
use crate::models::utils::{impl_updated_at_cb, sanitize_description_cb, updated_at_cb_fn};
use crate::models::workflow::DeleteWorkflow;

pub mod create;
pub mod update;

pub enum ContributionRequestStatus {
    WorkInProgress,
    Published,
    Merged,
    Closed,
}

impl ContributionRequestStatus {
    pub fn default() -> Option<String> {
        Some(ContributionRequestStatus::WorkInProgress.to_string())
    }
}

impl fmt::Display for ContributionRequestStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ContributionRequestStatus::WorkInProgress => {
                write!(f, "WorkInProgress")
            }
            ContributionRequestStatus::Published => write!(f, "Published"),
            ContributionRequestStatus::Merged => write!(f, "Merged"),
            ContributionRequestStatus::Closed => write!(f, "Closed"),
        }
    }
}

#[charybdis_model(
    table_name = contribution_requests,
    partition_keys = [node_id],
    clustering_keys = [id],
    global_secondary_indexes = []
)]
#[derive(Serialize, Deserialize, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ContributionRequest {
    pub node_id: Uuid,

    #[serde(default = "Uuid::new_v4")]
    pub id: Uuid,

    pub root_id: Uuid,

    pub editor_ids: Option<Set<Uuid>>,

    #[serde(default)]
    pub title: Text,

    pub description: Option<Text>,

    #[serde(default = "chrono::Utc::now")]
    pub created_at: Timestamp,

    #[serde(default = "chrono::Utc::now")]
    pub updated_at: Timestamp,

    #[serde(default = "ContributionRequestStatus::default")]
    pub status: Option<Text>,

    #[serde(default)]
    pub owner_id: Uuid,

    pub owner: Option<Frozen<Profile>>,

    #[charybdis(ignore)]
    #[serde(skip)]
    pub branch: Option<Branch>,

    #[charybdis(ignore)]
    #[serde(skip)]
    pub node: Option<Node>,
}

impl Callbacks for ContributionRequest {
    type Extension = RequestData;
    type Error = NodecosmosError;

    async fn before_insert(&mut self, _db_session: &CachingSession, data: &RequestData) -> Result<(), NodecosmosError> {
        self.set_defaults(data);
        self.create_branch(data).await?;
        self.create_branch_node(data).await?;

        Ok(())
    }

    async fn after_insert(&mut self, _: &CachingSession, data: &RequestData) -> Result<(), Self::Error> {
        let id = self.id;
        let title = self.title.clone();
        let owner = self.owner.clone();
        let data = data.clone();
        let node = self.node(data.db_session()).await?;
        let node_original_id = node.original_id();
        let node_id = node.id;
        let node_owner_id = node.owner_id;
        let node_editor_ids = node.editor_ids.clone();

        tokio::spawn(async move {
            let notification = Notification::new(
                NotificationType::NewContributionRequest,
                format!("created new contribution request - {}", title),
                format!(
                    "{client_url}/nodes/{original_id}/{node_id}/contribution_requests/{id}",
                    client_url = &data.app.config.client_url,
                    original_id = node_original_id,
                    node_id = node_id,
                    id = id
                ),
                owner,
            );
            let mut receiver_ids = HashSet::new();
            receiver_ids.insert(node_owner_id);

            if let Some(editor_ids) = node_editor_ids {
                receiver_ids.extend(editor_ids);
            }

            let _ = notification
                .create_for_receivers(&data, receiver_ids)
                .await
                .map_err(|e| {
                    log::error!("Error creating notification for new contribution request: {:?}", e);
                });
        });

        Ok(())
    }

    updated_at_cb_fn!();

    async fn after_delete(&mut self, db_session: &CachingSession, _: &RequestData) -> Result<(), Self::Error> {
        self.branch(db_session).await?.delete().execute(db_session).await?;

        // delete branch data
        let nodes = PkNode {
            branch_id: self.id,
            ..Default::default()
        };

        let workflow = DeleteWorkflow {
            branch_id: self.id,
            ..Default::default()
        };

        let flow = PkFlow {
            branch_id: self.id,
            ..Default::default()
        };

        let step = PkFlowStep {
            branch_id: self.id,
            ..Default::default()
        };

        let io = DeleteIo {
            branch_id: self.id,
            ..Default::default()
        };

        let description = Description {
            branch_id: self.id,
            ..Default::default()
        };

        let mut batch = CharybdisBatch::new();

        let _ = batch
            .append(nodes.delete_by_partition_key())
            .append(workflow.delete_by_partition_key())
            .append(flow.delete_by_partition_key())
            .append(step.delete_by_partition_key())
            .append(io.delete_by_partition_key())
            .append(description.delete_by_partition_key())
            .execute(db_session)
            .await
            .map_err(|e| {
                log::error!("Error deleting branch data: {:?}", e);
            });

        Ok(())
    }
}

impl ContributionRequest {
    pub async fn init_node(&mut self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        let node = Node::find_by_branch_id_and_id(self.original_id(), self.node_id)
            .execute(db_session)
            .await?;
        self.node = Some(node);

        Ok(())
    }

    pub async fn node(&mut self, db_session: &CachingSession) -> Result<&mut Node, NodecosmosError> {
        if self.node.is_none() {
            self.init_node(db_session).await?;
        }

        Ok(self.node.as_mut().expect("Node should be initialized"))
    }

    pub async fn branch(&mut self, db_session: &CachingSession) -> Result<&Branch, NodecosmosError> {
        if self.branch.is_none() {
            let branch = Branch::find_by_id(self.id).execute(db_session).await?;
            self.branch.replace(branch);
        }

        Ok(self.branch.as_ref().unwrap())
    }

    pub async fn merge(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        if self.status == Some(ContributionRequestStatus::Merged.to_string()) {
            return Err(NodecosmosError::PreconditionFailed(
                "Contribution request is already merged",
            ));
        }

        let branch = self.branch(data.db_session()).await?.clone();

        let updated_branch = branch.merge(data).await;

        match updated_branch {
            Ok(updated_branch) => {
                self.branch.replace(updated_branch);
                self.update_status(data, ContributionRequestStatus::Merged).await?;
            }
            Err(merge_error) => {
                self.branch.replace(merge_error.branch);
                return Err(merge_error.inner);
            }
        }

        Ok(())
    }
}

partial_contribution_request!(BaseContributionRequest, node_id, id, owner, title, created_at, status);

partial_contribution_request!(UpdateContributionRequestTitle, node_id, id, title, updated_at);

impl_updated_at_cb!(UpdateContributionRequestTitle);

partial_contribution_request!(
    UpdateContributionRequestDescription,
    node_id,
    id,
    description,
    updated_at
);

sanitize_description_cb!(UpdateContributionRequestDescription);
