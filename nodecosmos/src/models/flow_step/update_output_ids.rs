use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::branch::update::BranchUpdate;
use crate::models::branch::Branch;
use crate::models::flow_step::{FlowStep, UpdateOutputIdsFlowStep};
use crate::models::traits::{Branchable, FindOriginalOrBranched, Merge, ModelBranchParams};

impl UpdateOutputIdsFlowStep {
    pub async fn update_branch(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        Branch::update(data.db_session(), self.branch_id, BranchUpdate::EditNode(self.node_id)).await?;

        // we always compare against original if it exists
        let current = FlowStep::find_original_or_branched(
            data.db_session(),
            ModelBranchParams {
                original_id: self.original_id(),
                branch_id: self.branch_id,
                id: self.id,
            },
        )
        .await?;

        let [created_ids_by_node_id, removed_ids_by_node_id] =
            FlowStep::ios_diff(current.output_ids_by_node_id.clone(), &self.output_ids_by_node_id);

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

        // TODO: see nodecosmos/src/models/node/create.rs:258
        if current.is_original() {
            current.save_original_data_to_branch(data, self.branch_id).await?;

            self.output_ids_by_node_id.merge_unique(current.output_ids_by_node_id);
        }

        Ok(())
    }
}
