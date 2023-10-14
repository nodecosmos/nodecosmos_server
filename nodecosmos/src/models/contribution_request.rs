pub mod status;

use crate::errors::NodecosmosError;
use crate::models::commit::Commit;
use crate::models::helpers::{impl_updated_at_cb, sanitize_description_cb};
use crate::models::udts::Owner;
use charybdis::callbacks::Callbacks;
use charybdis::macros::charybdis_model;
use charybdis::types::{List, Text, Timestamp, Uuid};
use scylla::CachingSession;
use serde::{Deserialize, Serialize};

#[charybdis_model(
    table_name = contribution_requests,
    partition_keys = [node_id],
    clustering_keys = [id],
    secondary_indexes = []
)]
#[derive(Serialize, Deserialize, Default)]
pub struct ContributionRequest {
    #[serde(rename = "nodeId")] // node where the contribution request was created
    pub node_id: Uuid,

    #[serde(default = "Uuid::new_v4")]
    pub id: Uuid,

    #[serde(rename = "ownerId")]
    pub owner_id: Option<Uuid>,

    #[serde(rename = "editorIds")]
    pub editor_ids: Option<List<Uuid>>,

    pub owner: Option<Owner>,

    pub title: Option<Text>,
    pub description: Option<Text>,

    #[serde(rename = "descriptionMarkdown")]
    pub description_markdown: Option<Text>,

    #[serde(rename = "commitIds")]
    pub commit_ids: Option<List<Uuid>>,

    #[serde(rename = "createdAt")]
    pub created_at: Option<Timestamp>,

    #[serde(rename = "updatedAt")]
    pub updated_at: Option<Timestamp>,

    #[serde(default = "status::default_status")]
    pub status: Option<Text>,
}

impl ContributionRequest {
    pub fn set_owner(&mut self, owner: Owner) {
        self.owner_id = Some(owner.id);
        self.owner = Some(owner);
    }
}

impl Callbacks<NodecosmosError> for ContributionRequest {
    async fn before_insert(&mut self, _session: &CachingSession) -> Result<(), NodecosmosError> {
        let now = chrono::Utc::now();

        self.id = Uuid::new_v4();
        self.created_at = Some(now);
        self.updated_at = Some(now);

        Ok(())
    }

    async fn after_delete(&mut self, session: &CachingSession) -> Result<(), NodecosmosError> {
        Commit::delete_contribution_request_commits(session, self.id).await?;
        Ok(())
    }
}

partial_contribution_request!(
    BaseContributionRequest,
    node_id,
    id,
    owner,
    title,
    created_at,
    status
);

partial_contribution_request!(
    UpdateContributionRequestTitle,
    node_id,
    id,
    owner_id,
    title,
    updated_at
);
impl_updated_at_cb!(UpdateContributionRequestTitle);

partial_contribution_request!(
    UpdateContributionRequestDescription,
    node_id,
    id,
    owner_id,
    description,
    description_markdown,
    updated_at
);
sanitize_description_cb!(UpdateContributionRequestDescription);
