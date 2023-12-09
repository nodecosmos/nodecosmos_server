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
}
