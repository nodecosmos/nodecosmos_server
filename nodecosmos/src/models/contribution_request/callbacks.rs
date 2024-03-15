use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::contribution_request::{
    ContributionRequest, UpdateContributionRequestDescription, UpdateContributionRequestTitle,
};
use crate::models::utils::{impl_updated_at_cb, sanitize_description_cb};
use charybdis::callbacks::Callbacks;
use scylla::CachingSession;

impl Callbacks for ContributionRequest {
    type Extension = RequestData;
    type Error = NodecosmosError;

    async fn before_insert(&mut self, _session: &CachingSession, data: &RequestData) -> Result<(), NodecosmosError> {
        self.set_defaults(data);
        self.create_branch(data).await?;
        self.create_branch_node(data).await?;

        Ok(())
    }

    async fn before_update(&mut self, _: &CachingSession, _: &Self::Extension) -> Result<(), NodecosmosError> {
        self.updated_at = Some(chrono::Utc::now());

        Ok(())
    }
}

impl_updated_at_cb!(UpdateContributionRequestTitle);

sanitize_description_cb!(UpdateContributionRequestDescription);
