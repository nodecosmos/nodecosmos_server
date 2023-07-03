use charybdis::*;

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
    pub title: Option<Text>,
    pub description: Option<Text>,

    #[serde(rename = "descriptionMarkdown")]
    pub description_markdown: Option<Text>,

    #[serde(rename = "createdAt")]
    pub created_at: Option<Timestamp>,

    #[serde(rename = "updatedAt")]
    pub updated_at: Option<Timestamp>,

    pub open: Option<Boolean>,
    pub merged: Option<Boolean>,
}
