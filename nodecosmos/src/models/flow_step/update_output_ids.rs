use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::branch::update::BranchUpdate;
use crate::models::branch::Branch;
use crate::models::flow_step::{FlowStep, UpdateOutputIdsFlowStep};
use crate::models::traits::{Branchable, FindOrInsertBranched, ModelBranchParams};

impl UpdateOutputIdsFlowStep {
    pub async fn update_branch(&self, data: &RequestData) -> Result<(), NodecosmosError> {
        Branch::update(data.db_session(), self.branch_id, BranchUpdate::EditNode(self.node_id)).await?;

        let current = FlowStep::find_or_insert_branched(
            data,
            ModelBranchParams {
                original_id: self.original_id(),
                branch_id: self.branch_id,
                id: self.id,
            },
        )
        .await?;

        current.preserve_flow_step_outputs(data).await?;

        let [created_ids_by_node_id, removed_ids_by_node_id] =
            FlowStep::ios_diff(current.output_ids_by_node_id, &self.output_ids_by_node_id);

        Branch::update(
            data.db_session(),
            self.branch_id,
            BranchUpdate::CreateFlowStepOutputs((self.id, created_ids_by_node_id)),
        )
        .await?;

        Branch::update(
            data.db_session(),
            self.branch_id,
            BranchUpdate::DeletedFlowStepOutputs((self.id, removed_ids_by_node_id)),
        )
        .await?;

        Ok(())
    }
}
