use crate::errors::NodecosmosError;
use charybdis::model::Model;
use charybdis::operations::Find;
use scylla::CachingSession;

pub trait Reload: Model {
    async fn reload(&mut self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        *self = self.find_by_primary_key().execute(db_session).await?;

        Ok(())
    }
}

impl<T: Model> Reload for T {}
