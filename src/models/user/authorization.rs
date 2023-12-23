use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::authorization::Authorization;
use crate::models::user::User;
use charybdis::types::{Set, Uuid};

impl Authorization for User {
    fn is_public(&self) -> bool {
        false
    }

    fn owner_id(&self) -> Option<Uuid> {
        Some(self.id)
    }

    fn editor_ids(&self) -> Option<Set<Uuid>> {
        None
    }

    async fn auth_creation(&mut self, _data: &RequestData) -> Result<(), NodecosmosError> {
        Ok(())
    }
}
