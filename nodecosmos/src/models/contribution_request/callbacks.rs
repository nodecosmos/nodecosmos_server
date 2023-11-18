use crate::errors::NodecosmosError;
use crate::models::contribution_request::{
    ContributionRequest, UpdateContributionRequestDescription, UpdateContributionRequestTitle,
};
use crate::models::utils::{impl_updated_at_cb, sanitize_description_cb};
use charybdis::callbacks::Callbacks;
use charybdis::types::Uuid;
use scylla::CachingSession;

impl Callbacks for ContributionRequest {
    type Error = NodecosmosError;

    async fn before_insert(&mut self, _session: &CachingSession) -> Result<(), NodecosmosError> {
        let now = chrono::Utc::now();

        self.id = Uuid::new_v4();
        self.created_at = Some(now);
        self.updated_at = Some(now);

        Ok(())
    }
}

impl_updated_at_cb!(UpdateContributionRequestTitle);

sanitize_description_cb!(UpdateContributionRequestDescription);
