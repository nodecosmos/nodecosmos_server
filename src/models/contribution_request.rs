mod authorization;
mod callbacks;
pub mod create;
pub mod status;
pub mod update;

use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::branch::branchable::Branchable;
use crate::models::branch::Branch;
use crate::models::contribution_request::status::ContributionRequestStatus;
use crate::models::node::Node;
use crate::models::udts::Owner;
use charybdis::macros::charybdis_model;
use charybdis::operations::{Update, UpdateWithExtCallbacks};
use charybdis::types::{Frozen, Set, Text, Timestamp, Uuid};
use scylla::CachingSession;
use serde::{Deserialize, Serialize};

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

    #[serde(default = "status::default_status")]
    pub status: Option<Text>,

    #[serde(default, rename = "ownerId")]
    pub owner_id: Uuid,

    pub owner: Option<Frozen<Owner>>,

    #[charybdis(ignore)]
    #[serde(skip)]
    pub branch: Option<Branch>,

    #[charybdis(ignore)]
    #[serde(skip)]
    pub node: Option<Node>,
}

impl ContributionRequest {
    pub async fn init_node(&mut self, session: &CachingSession) -> Result<(), NodecosmosError> {
        let node = Node::find_by_id_and_branch_id(session, self.node_id, self.node_id).await?;
        self.node = Some(node);

        Ok(())
    }

    pub async fn node(&mut self, session: &CachingSession) -> Result<&mut Node, NodecosmosError> {
        if self.node.is_none() {
            self.init_node(session).await?;
        }

        Ok(self.node.as_mut().unwrap())
    }

    pub async fn merge(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        if self.status == Some(ContributionRequestStatus::Merged.to_string()) {
            return Err(NodecosmosError::PreconditionFailed(
                "Contribution request is already merged",
            ));
        }

        let mut branch = self.branch(data.db_session()).await?;

        branch.merge(data).await?;

        self.update_status(data, ContributionRequestStatus::Merged).await?;

        Ok(())
    }
}

impl Branchable for ContributionRequest {
    fn id(&self) -> Uuid {
        self.id
    }

    fn branch_id(&self) -> Uuid {
        self.id
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
