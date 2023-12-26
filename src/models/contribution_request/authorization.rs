use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::authorization::Authorization;
use crate::models::branch::branchable::Branchable;
use crate::models::contribution_request::ContributionRequest;
use charybdis::types::{Set, Uuid};
use serde_json::json;

impl Authorization for ContributionRequest {
    async fn before_auth(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        let branch = self.branch(data.db_session()).await?;
        self.branch = Some(branch);

        Ok(())
    }

    fn is_public(&self) -> bool {
        self.branch.as_ref().map_or(false, |branch| branch.is_public)
    }

    fn owner_id(&self) -> Option<Uuid> {
        self.branch.as_ref().map(|branch| branch.owner_id)
    }

    fn editor_ids(&self) -> Option<Set<Uuid>> {
        self.branch
            .as_ref()
            .map(|branch| branch.editor_ids.clone().unwrap_or_default())
    }

    async fn auth_creation(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        let node = self.node(data.db_session()).await?;

        if node.is_public {
            return Ok(());
        }

        node.auth_update(data).await?;

        Ok(())
    }
}
