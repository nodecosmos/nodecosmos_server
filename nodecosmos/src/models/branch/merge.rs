mod flow_steps;
mod flows;
mod ios;
mod nodes;
mod workflow;

use crate::api::data::RequestData;
use crate::api::types::ActionTypes;
use crate::errors::NodecosmosError;
use crate::models::branch::merge::flows::MergeFlows;
use crate::models::branch::merge::ios::MergeIos;
use crate::models::branch::merge::nodes::MergeNodes;
use crate::models::branch::{Branch, BranchStatus};
use crate::models::traits::Branchable;
use crate::models::utils::file::read_file_names;
use charybdis::operations::{Update, UpdateWithCallbacks};
use log::{error, warn};
use serde::{Deserialize, Serialize};
use std::fs::create_dir_all;

const RECOVERY_DATA_DIR: &str = "tmp/merge-recovery";
const RECOVER_FILE_PREFIX: &str = "merge_recovery_data";

pub struct MergeError {
    pub inner: NodecosmosError,
    pub branch: Branch,
}

#[derive(Clone, Copy, Serialize, Deserialize, PartialOrd, PartialEq, Debug)]
pub enum MergeStep {
    Start = 0,
    RestoreNodes = 1,
    CreateNodes = 2,
    DeleteNodes = 3,
    ReorderNodes = 4,
    UpdateNodesTitles = 5,
    UpdateNodesDescription = 6,
    UpdateWorkflowInitialInputs = 7,
    RestoreFlows = 8,
    CreateFlows = 9,
    DeleteFlows = 10,
    UpdateFlowsTitles = 11,
    UpdateFlowsDescription = 12,
    RestoreFlowSteps = 13,
    CreateFlowSteps = 14,
    DeleteFlowSteps = 15,
    CreateFlowStepNodes = 16,
    DeleteFlowStepNodes = 17,
    CreateFlowStepInputs = 18,
    DeleteFlowStepInputs = 19,
    CreateFlowStepOutputs = 20,
    DeleteFlowStepOutputs = 21,
    EditFlowStepsDescription = 22,
    RestoreIos = 23,
    CreateIos = 24,
    DeleteIos = 25,
    UpdateIoTitles = 26,
    UpdateIosDescription = 27,
    Finish = 28,
}

impl MergeStep {
    pub fn increment(&mut self) {
        *self = MergeStep::from(*self as u8 + 1);
    }

    pub fn decrement(&mut self) {
        *self = MergeStep::from(*self as u8 - 1);
    }
}

impl From<u8> for MergeStep {
    fn from(value: u8) -> Self {
        match value {
            0 => MergeStep::Start,
            1 => MergeStep::RestoreNodes,
            2 => MergeStep::CreateNodes,
            3 => MergeStep::DeleteNodes,
            4 => MergeStep::ReorderNodes,
            5 => MergeStep::UpdateNodesTitles,
            6 => MergeStep::UpdateNodesDescription,
            7 => MergeStep::UpdateWorkflowInitialInputs,
            8 => MergeStep::RestoreFlows,
            9 => MergeStep::CreateFlows,
            10 => MergeStep::DeleteFlows,
            11 => MergeStep::UpdateFlowsTitles,
            12 => MergeStep::UpdateFlowsDescription,
            13 => MergeStep::RestoreFlowSteps,
            14 => MergeStep::CreateFlowSteps,
            15 => MergeStep::DeleteFlowSteps,
            16 => MergeStep::CreateFlowStepNodes,
            17 => MergeStep::DeleteFlowStepNodes,
            18 => MergeStep::CreateFlowStepInputs,
            19 => MergeStep::DeleteFlowStepInputs,
            20 => MergeStep::CreateFlowStepOutputs,
            21 => MergeStep::DeleteFlowStepOutputs,
            22 => MergeStep::EditFlowStepsDescription,
            23 => MergeStep::RestoreIos,
            24 => MergeStep::CreateIos,
            25 => MergeStep::DeleteIos,
            26 => MergeStep::UpdateIoTitles,
            27 => MergeStep::UpdateIosDescription,
            28 => MergeStep::Finish,
            _ => panic!("Invalid merge step value: {}", value),
        }
    }
}

/// ScyllaDB does not support transactions like SQL databases. We need to introduce a pattern to handle merge failures.
/// We use the SAGA pattern to handle merge failures.
/// We store the state of the merge process in a file and recover from it.
#[derive(Serialize, Deserialize)]
pub struct BranchMerge {
    branch: Branch,
    merge_step: MergeStep,
}

impl BranchMerge {
    pub async fn run(branch: Branch, data: &RequestData) -> Result<Self, MergeError> {
        let mut branch_merge = BranchMerge {
            branch,
            merge_step: MergeStep::Start,
        };

        match branch_merge.merge(data).await {
            Ok(_) => {
                branch_merge.branch.status = Some(BranchStatus::Merged.to_string());
                Ok(branch_merge)
            }
            Err(e) => match branch_merge.recover(data).await {
                Ok(_) => {
                    branch_merge.branch.status = Some(BranchStatus::Recovered.to_string());
                    Err(MergeError {
                        inner: e,
                        branch: branch_merge.branch,
                    })
                }
                Err(recovery_err) => {
                    branch_merge.branch.status = Some(BranchStatus::RecoveryFailed.to_string());

                    error!("Mere::Failed to recover: {}", recovery_err);
                    branch_merge.serialize_and_store_to_disk();

                    Err(MergeError {
                        inner: NodecosmosError::FatalMergeError(format!("Failed to merge and recover: {}", e)),
                        branch: branch_merge.branch,
                    })
                }
            },
        }
    }

    pub async fn recover_from_stored_data(data: &RequestData) {
        create_dir_all(RECOVERY_DATA_DIR).unwrap();
        let files = read_file_names(RECOVERY_DATA_DIR, RECOVER_FILE_PREFIX).await;

        for file in files {
            let serialized = std::fs::read_to_string(file.clone()).unwrap();
            let mut branch_merge: BranchMerge = serde_json::from_str(&serialized)
                .map_err(|err| {
                    error!(
                        "Error in deserializing recovery data from file {}: {}",
                        file.clone(),
                        err
                    );
                })
                .unwrap();
            std::fs::remove_file(file.clone()).unwrap();

            if let Err(err) = branch_merge.recover(data).await {
                error!("Error in recovery from file {}: {}", file, err);
                branch_merge.serialize_and_store_to_disk();
                continue;
            }

            branch_merge.unlock_resource(data).await.expect(
                format!(
                    "Merge Recovery: Failed to unlock resource: branch_id: {}",
                    branch_merge.branch.id
                )
                .as_str(),
            );
        }
    }

    /// Merge the branch
    async fn merge(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        let mut merge_nodes = MergeNodes::new(&self.branch, data).await?;
        let mut merge_ios = MergeIos::new(&self.branch, data).await?;
        let mut merge_flows = MergeFlows::new(&self.branch, data).await?;

        while self.merge_step < MergeStep::Finish {
            match self.merge_step {
                MergeStep::RestoreNodes => merge_nodes.restore_nodes(data).await?,
                MergeStep::CreateNodes => merge_nodes.create_nodes(data).await?,
                MergeStep::DeleteNodes => merge_nodes.delete_nodes(data).await?,
                MergeStep::ReorderNodes => merge_nodes.reorder_nodes(data).await?,
                MergeStep::UpdateNodesTitles => merge_nodes.update_nodes_titles(data).await?,
                MergeStep::UpdateNodesDescription => merge_nodes.update_nodes_description(data).await?,
                MergeStep::RestoreIos => merge_ios.restore_ios(data).await?,
                MergeStep::CreateIos => merge_ios.create_ios(data).await?,
                MergeStep::DeleteIos => merge_ios.delete_ios(data).await?,
                MergeStep::UpdateIoTitles => merge_ios.update_ios_titles(data).await?,
                MergeStep::UpdateIosDescription => merge_ios.update_ios_description(data).await?,

                _ => {}
            }

            self.merge_step.increment();
        }

        Ok(())
    }

    /// Recover from merge failure in reverse order
    pub async fn recover(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        let mut merge_nodes = MergeNodes::new(&self.branch, data).await?;
        let mut merge_ios = MergeIos::new(&self.branch, data).await?;

        while self.merge_step > MergeStep::Start {
            match self.merge_step {
                MergeStep::RestoreNodes => merge_nodes.undo_restore_nodes(data).await?,
                MergeStep::CreateNodes => merge_nodes.undo_create_nodes(data).await?,
                MergeStep::DeleteNodes => merge_nodes.undo_delete_nodes(data).await?,
                MergeStep::ReorderNodes => merge_nodes.undo_reorder_nodes(data).await?,
                MergeStep::UpdateNodesTitles => merge_nodes.undo_update_nodes_titles(data).await?,
                MergeStep::UpdateNodesDescription => merge_nodes.undo_update_nodes_description(data).await?,
                MergeStep::RestoreIos => merge_ios.undo_restore_ios(data).await?,
                MergeStep::CreateIos => merge_ios.undo_create_ios(data).await?,
                MergeStep::DeleteIos => merge_ios.undo_delete_ios(data).await?,
                MergeStep::UpdateIoTitles => merge_ios.undo_update_ios_titles(data).await?,
                MergeStep::UpdateIosDescription => merge_ios.undo_update_ios_description(data).await?,
                _ => {}
            }

            self.merge_step.decrement();
        }

        Ok(())
    }

    async fn unlock_resource(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        let node = self.branch.node(data.db_session()).await?;

        data.resource_locker()
            .unlock_resource(node.root_id, node.branchise_id(node.root_id))
            .await?;
        data.resource_locker()
            .unlock_resource_action(ActionTypes::Merge, node.root_id, node.branchise_id(node.root_id))
            .await?;

        Ok(())
    }

    fn serialize_and_store_to_disk(&self) {
        // serialize branch_merge and store to disk
        let serialized = serde_json::to_string(self).unwrap();
        let filename = format!("{}{}.json", RECOVER_FILE_PREFIX, self.branch.id);
        let path = format!("{}/{}", RECOVERY_DATA_DIR, filename);
        let res = std::fs::write(path.clone(), serialized);

        match res {
            Ok(_) => warn!("Merge Recovery data saved to file: {}", path),
            Err(err) => error!("Error in saving recovery data: {}", err),
        }
    }
}

impl Branch {
    pub async fn merge(mut self, data: &RequestData) -> Result<Self, MergeError> {
        if let Err(e) = self.validate_no_existing_conflicts().await {
            return Err(MergeError { inner: e, branch: self });
        }

        if let Err(e) = self.check_conflicts(data.db_session()).await {
            return Err(MergeError { inner: e, branch: self });
        }

        let merge = BranchMerge::run(self, data).await?;

        let res = merge.branch.update().execute(data.db_session()).await;

        if let Err(e) = res {
            return Err(MergeError {
                inner: NodecosmosError::from(e),
                branch: merge.branch,
            });
        }

        Ok(merge.branch)
    }
}
