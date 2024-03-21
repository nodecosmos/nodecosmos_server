use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::traits::Branchable;
use crate::models::workflow::Workflow;
use charybdis::operations::{Find, Insert};

impl Workflow {
    pub async fn create_branched_if_not_exist(&self, data: &RequestData) -> Result<(), NodecosmosError> {
        let maybe_branched = self.maybe_find_by_primary_key().execute(data.db_session()).await?;

        if maybe_branched.is_none() {
            let mut workflow = Workflow {
                node_id: self.node_id,
                branch_id: self.original_id(),
                id: self.id,
                ..Default::default()
            }
            .find_by_primary_key()
            .execute(data.db_session())
            .await?;

            workflow.branch_id = self.branch_id;

            workflow.insert().execute(data.db_session()).await?;

            return Ok(());
        }

        Ok(())
    }

    pub async fn create_branched_if_original_exists(&self, data: &RequestData) -> Result<(), NodecosmosError> {
        let mut maybe_original = Workflow {
            node_id: self.node_id,
            branch_id: self.original_id(),
            id: self.id,
            ..Default::default()
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
