use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::branch::Branch;
use crate::models::contribution_request::ContributionRequest;
use crate::models::traits::Branchable;
use crate::models::udts::Profile;
use charybdis::operations::Insert;
use charybdis::types::Uuid;

impl ContributionRequest {
    pub fn set_defaults(&mut self, data: &RequestData) {
        let now = chrono::Utc::now();
        let owner = Profile::init_from_current_user(&data.current_user);

        self.id = Uuid::new_v4();
        self.created_at = Some(now);
        self.updated_at = Some(now);

        self.owner_id = owner.id;
        self.owner = Some(owner);
    }

    pub async fn create_branch_node(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        let mut node = self.node(data.db_session()).await?.clone();

        node.branch_id = self.id;

        node.insert().execute(data.db_session()).await?;

        Ok(())
    }

    pub async fn create_branch(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        let is_public = self.node(data.db_session()).await?.is_public;

        let branch = Branch {
            id: self.branch_id(),
            title: self.title.clone(),
            description: self.description.clone(),
            owner_id: self.owner_id,
            owner: self.owner.clone(),
            is_public,
            is_contribution_request: Some(true),
            node_id: self.node_id,
            ..Default::default()
        };

        branch.insert().execute(data.db_session()).await?;

        Ok(())
    }
}
