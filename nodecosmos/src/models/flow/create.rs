use charybdis::operations::{Find, Insert};

use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::branch::update::BranchUpdate;
use crate::models::branch::Branch;
use crate::models::flow::Flow;
use crate::models::node::Node;
use crate::models::traits::{Branchable, FindOrInsertBranched};

impl Flow {
    pub async fn create_branched_if_original_exists(&self, data: &RequestData) -> Result<(), NodecosmosError> {
        if self.is_branched() {
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
            }
        }

        Ok(())
    }

    pub async fn preserve_branch_node(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        if self.is_branched() {
            Node::find_or_insert_branched(data, self.node_id, self.branch_id).await?;
        }

        Ok(())
    }

    pub async fn update_branch_with_creation(&self, data: &RequestData) -> Result<(), NodecosmosError> {
        if self.is_branched() {
            Branch::update(data, self.branch_id, BranchUpdate::EditNodeWorkflow(self.node_id)).await?;
            Branch::update(data, self.branch_id, BranchUpdate::CreateFlow(self.id)).await?;
        }

        Ok(())
    }

    pub async fn update_branch_with_deletion(&self, data: &RequestData) -> Result<(), NodecosmosError> {
        if self.is_branched() {
            Branch::update(data, self.branch_id, BranchUpdate::EditNodeWorkflow(self.node_id)).await?;
            Branch::update(data, self.branch_id, BranchUpdate::DeleteFlow(self.id)).await?;
        }

        Ok(())
    }
}
