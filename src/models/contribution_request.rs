mod callbacks;
pub mod create;
pub mod update;

use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::branch::Branch;
use crate::models::node::Node;
use crate::models::udts::Profile;
use charybdis::macros::charybdis_model;
use charybdis::types::{Frozen, Set, Text, Timestamp, Uuid};
use scylla::CachingSession;
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;

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
#[derive(Serialize, Deserialize, Default)]
pub struct ContributionRequest {
    #[serde(rename = "nodeId")]
    pub node_id: Uuid,

    #[serde(default = "Uuid::new_v4")]
    pub id: Uuid,

    #[serde(rename = "editorIds")]
    pub editor_ids: Option<Set<Uuid>>,

    pub title: Option<Text>,
    pub description: Option<Text>,

    #[serde(rename = "descriptionMarkdown")]
    pub description_markdown: Option<Text>,

    #[serde(rename = "createdAt")]
    pub created_at: Option<Timestamp>,

    #[serde(rename = "updatedAt")]
    pub updated_at: Option<Timestamp>,

    #[serde(default = "ContributionRequestStatus::default")]
    pub status: Option<Text>,

    #[serde(default, rename = "ownerId")]
    pub owner_id: Uuid,

    pub owner: Option<Frozen<Profile>>,

    #[charybdis_model(ignore)]
    #[serde(skip)]
    pub branch: Rc<RefCell<Option<Branch>>>,

    #[charybdis_model(ignore)]
    #[serde(skip)]
    pub node: Option<Node>,
}

impl ContributionRequest {
    pub async fn init_node(&mut self, session: &CachingSession) -> Result<(), NodecosmosError> {
        let node = Node::find_by_id_and_branch_id(self.node_id, self.node_id)
            .execute(session)
            .await?;
        self.node = Some(node);

        Ok(())
    }

    pub async fn node(&mut self, session: &CachingSession) -> Result<&mut Node, NodecosmosError> {
        if self.node.is_none() {
            self.init_node(session).await?;
        }

        Ok(self.node.as_mut().unwrap())
    }

    pub async fn branch(&self, session: &CachingSession) -> Result<Branch, NodecosmosError> {
        let mut branch_ref = self.branch.borrow_mut();
        if branch_ref.is_none() {
            *branch_ref = Some(Branch::find_by_id(self.id).execute(session).await?);
        }

        branch_ref.clone().ok_or(NodecosmosError::InternalServerError(
            "Branch not found for contribution request".to_string(),
        ))
    }
    pub async fn merge(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        if self.status == Some(ContributionRequestStatus::Merged.to_string()) {
            return Err(NodecosmosError::PreconditionFailed(
                "Contribution request is already merged",
            ));
        }

        let branch = self.branch(data.db_session()).await?;

        branch.merge(data).await?;

        self.update_status(data, ContributionRequestStatus::Merged).await?;

        Ok(())
    }
}

partial_contribution_request!(BaseContributionRequest, node_id, id, owner, title, created_at, status);

partial_contribution_request!(UpdateContributionRequestTitle, node_id, id, title, updated_at);

partial_contribution_request!(
    UpdateContributionRequestDescription,
    node_id,
    id,
    description,
    description_markdown,
    updated_at
);
