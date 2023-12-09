use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::branch::branchable::Branchable;
use crate::models::branch::Branch;
use crate::models::contribution_request::ContributionRequest;
use crate::models::udts::Owner;
use charybdis::operations::Insert;
use charybdis::types::Uuid;

impl ContributionRequest {
    pub fn set_defaults(&mut self) {
        let now = chrono::Utc::now();

        self.id = Uuid::new_v4();
        self.created_at = Some(now);
        self.updated_at = Some(now);
    }

    pub async fn create_branch_node(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        let mut node = self.node(data.db_session()).await?.clone();

        node.branch_id = self.id;

        node.insert(data.db_session()).await?;

        Ok(())
    }

    pub async fn create_branch(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        let owner = Owner::init(&data.current_user);
        let is_public = self.node(data.db_session()).await?.is_public;

        let branch = Branch {
            id: self.branch_id(),
            title: self.title.clone(),
            description: self.description.clone(),
            owner_id: owner.id,
            owner: Some(owner),
            is_public,
            is_contribution_request: Some(true),
            ..Default::default()
        };

        branch.insert(data.db_session()).await?;

        Ok(())
    }
}
