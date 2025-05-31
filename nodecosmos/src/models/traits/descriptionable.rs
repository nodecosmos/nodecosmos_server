use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::description::Description;
use crate::models::traits::{Branchable, Id};
use charybdis::operations::DeleteWithCallbacks;
use scylla::client::caching_session::CachingSession;

pub trait Descriptionable: Id + Branchable {
    async fn maybe_description(&self, db_session: &CachingSession) -> Result<Option<Description>, NodecosmosError> {
        Description::maybe_find_first_by_branch_id_and_object_id(self.branch_id(), self.id())
            .execute(db_session)
            .await
            .map_err(NodecosmosError::from)
    }

    async fn delete_description(&self, data: &RequestData) -> Result<(), NodecosmosError> {
        let description = self.maybe_description(data.db_session()).await?;

        if let Some(mut description) = description {
            description.delete_cb(data).execute(data.db_session()).await?;
        }

        Ok(())
    }
}

impl<T: Id + Branchable> Descriptionable for T {}
