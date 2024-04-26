use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::branch::update::BranchUpdate;
use crate::models::branch::Branch;
use crate::models::flow::{Flow, UpdateTitleFlow};
use crate::models::traits::{Branchable, FindOrInsertBranched};

impl UpdateTitleFlow {
    pub async fn update_branch(&self, data: &RequestData) -> Result<(), NodecosmosError> {
        if self.is_branched() {
            Branch::update(data, self.branch_id, BranchUpdate::EditNodeWorkflow(self.node_id)).await?;
            Flow::find_or_insert_branched(data, self.node_id, self.branch_id, self.id).await?;
            Branch::update(data, self.branch_id, BranchUpdate::EditFlowTitle(self.id)).await?;
        }

        Ok(())
    }
}
