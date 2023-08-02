use crate::models::helpers::{impl_default_callbacks, impl_updated_at_cb, sanitize_description_cb};
use crate::models::udts::Owner;
use charybdis::{List, Text, Timestamp, Uuid};
use charybdis_macros::{charybdis_model, partial_model_generator};

#[partial_model_generator]
#[charybdis_model(
    table_name = contribution_requests,
    partition_keys = [node_id],
    clustering_keys = [id],
    secondary_indexes = []
)]
pub struct ContributionRequest {
    #[serde(rename = "nodeId")] // node where the contribution request was created
    pub node_id: Uuid,

    #[serde(default = "Uuid::new_v4")]
    pub id: Uuid,

    #[serde(rename = "ownerId")]
    pub owner_id: Uuid,

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

    pub status: Option<Text>,
}

impl_default_callbacks!(ContributionRequest);

partial_contribution_request!(
    BaseContributionRequest,
    node_id,
    id,
    owner_id,
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
