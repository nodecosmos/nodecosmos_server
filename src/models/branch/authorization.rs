use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::authorization::Authorization;
use crate::models::branch::Branch;
use charybdis::types::{Set, Uuid};

impl Authorization for Branch {
    fn is_public(&self) -> bool {
        self.is_public
    }

    fn owner_id(&self) -> Option<Uuid> {
        Some(self.owner_id)
    }

    fn editor_ids(&self) -> Option<Set<Uuid>> {
        self.editor_ids.clone()
    }

    async fn auth_creation(&mut self, _data: &RequestData) -> Result<(), NodecosmosError> {
        Err(NodecosmosError::Forbidden("Branches cannot be created".to_string()))
    }
}
