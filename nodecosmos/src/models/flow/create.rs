use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::flow::Flow;
use crate::models::traits::Branchable;
use charybdis::operations::{Find, Insert};

impl Flow {
    pub async fn create_branched_if_original_exists(&self, data: &RequestData) -> Result<(), NodecosmosError> {
        let mut maybe_original = Flow {
            branch_id: self.original_id(),
            ..self.clone()
        }
        .maybe_find_by_primary_key()
        .execute(data.db_session())
        .await?;

        if let Some(maybe_original) = maybe_original.as_mut() {
            maybe_original.branch_id = self.branch_id;

            maybe_original.insert().execute(data.db_session()).await?;

            return Ok(());
        }

        Ok(())
    }
}
