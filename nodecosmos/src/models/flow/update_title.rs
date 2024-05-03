use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::branch::update::BranchUpdate;
use crate::models::branch::Branch;
use crate::models::flow::{Flow, UpdateTitleFlow};
use crate::models::traits::{Branchable, FindOrInsertBranched, ModelBranchParams};

impl UpdateTitleFlow {
    pub async fn update_branch(&self, data: &RequestData) -> Result<(), NodecosmosError> {
        if self.is_branch() {
            Branch::update(data.db_session(), self.branch_id, BranchUpdate::EditNode(self.node_id)).await?;
            Flow::find_or_insert_branched(
                data,
                ModelBranchParams {
                    original_id: self.original_id(),
                    branch_id: self.branch_id,
                    node_id: self.node_id,
                    id: self.id,
                },
            )
            .await?;
            Branch::update(data.db_session(), self.branch_id, BranchUpdate::EditFlowTitle(self.id)).await?;
        }

        Ok(())
    }
}
