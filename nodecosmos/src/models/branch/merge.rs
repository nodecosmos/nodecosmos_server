use std::fs::create_dir_all;

use charybdis::operations::Update;
use log::{error, warn};
use serde::{Deserialize, Serialize};

use crate::api::data::RequestData;
use crate::api::types::ActionTypes;
use crate::errors::NodecosmosError;
use crate::models::branch::merge::conflicts::MergeConflicts;
use crate::models::branch::merge::descriptions::MergeDescriptions;
use crate::models::branch::merge::flow_steps::MergeFlowSteps;
use crate::models::branch::merge::flows::MergeFlows;
use crate::models::branch::merge::ios::MergeIos;
use crate::models::branch::merge::nodes::MergeNodes;
use crate::models::branch::merge::workflows::MergeWorkflows;
use crate::models::branch::{Branch, BranchStatus};
use crate::models::utils::file::read_file_names;

mod conflicts;
mod descriptions;
mod flow_steps;
mod flows;
mod ios;
mod nodes;
mod workflows;

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
    UpdateWorkflowInitialInputs = 6,
    RestoreFlows = 7,
    CreateFlows = 8,
    DeleteFlows = 9,
    UpdateFlowsTitles = 10,
    DeleteFlowSteps = 11,
    RestoreFlowSteps = 12,
    CreateFlowSteps = 13,
    CreateFlowStepNodes = 14,
    DeleteFlowStepNodes = 15,
    CreateFlowStepInputs = 16,
    DeleteFlowStepInputs = 17,
    CreateFlowStepOutputs = 18,
    DeleteFlowStepOutputs = 19,
    RestoreIos = 20,
    CreateIos = 21,
    DeleteIos = 22,
    UpdateIoTitles = 23,
    UpdateDescriptions = 24,
    DeleteDescriptions = 25,
    Finish = 26,
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
            6 => MergeStep::UpdateWorkflowInitialInputs,
            7 => MergeStep::RestoreFlows,
            8 => MergeStep::CreateFlows,
            9 => MergeStep::DeleteFlows,
            10 => MergeStep::UpdateFlowsTitles,
            11 => MergeStep::DeleteFlowSteps,
            12 => MergeStep::RestoreFlowSteps,
            13 => MergeStep::CreateFlowSteps,
            14 => MergeStep::CreateFlowStepNodes,
            15 => MergeStep::DeleteFlowStepNodes,
            16 => MergeStep::CreateFlowStepInputs,
            17 => MergeStep::DeleteFlowStepInputs,
            18 => MergeStep::CreateFlowStepOutputs,
            19 => MergeStep::DeleteFlowStepOutputs,
            20 => MergeStep::RestoreIos,
            21 => MergeStep::CreateIos,
            22 => MergeStep::DeleteIos,
            23 => MergeStep::UpdateIoTitles,
            24 => MergeStep::UpdateDescriptions,
            25 => MergeStep::DeleteDescriptions,
            26 => MergeStep::Finish,
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
    nodes: MergeNodes,
    workflows: MergeWorkflows,
    flows: MergeFlows,
    flow_steps: MergeFlowSteps,
    ios: MergeIos,
    descriptions: MergeDescriptions,
}

impl BranchMerge {
    pub async fn new(data: &RequestData, branch: Branch) -> Result<Self, MergeError> {
        let nodes = MergeNodes::new(&branch, data).await.map_err(|e| MergeError {
            inner: e,
            branch: branch.clone(),
        })?;

        let workflows = MergeWorkflows::new(&branch);

        let ios = MergeIos::new(&branch, data).await.map_err(|e| MergeError {
            inner: e,
            branch: branch.clone(),
        })?;

        let flows = MergeFlows::new(&branch, data).await.map_err(|e| MergeError {
            inner: e,
            branch: branch.clone(),
        })?;

        let flow_steps = MergeFlowSteps::new(&branch, data).await.map_err(|e| MergeError {
            inner: e,
            branch: branch.clone(),
        })?;

        let descriptions = MergeDescriptions::new(data.db_session(), &branch)
            .await
            .map_err(|e| MergeError {
                inner: e,
                branch: branch.clone(),
            })?;

        Ok(BranchMerge {
            branch,
            merge_step: MergeStep::Start,
            nodes,
            workflows,
            ios,
            flows,
            flow_steps,
            descriptions,
        })
    }

    pub async fn check_conflicts(mut self, data: &RequestData) -> Result<Self, MergeError> {
        if let Err(e) = MergeConflicts::new(&mut self).run_check(data).await {
            return Err(MergeError {
                inner: e,
                branch: self.branch,
            });
        }

        Ok(self)
    }

    pub async fn run(mut self, data: &RequestData) -> Result<Self, MergeError> {
        match self.merge(data).await {
            Ok(_) => {
                self.branch.status = Some(BranchStatus::Merged.to_string());
                Ok(self)
            }
            Err(e) => match self.recover(data).await {
                Ok(_) => {
                    self.branch.status = Some(BranchStatus::Recovered.to_string());
                    warn!("Merge::Recovered from error: {}", e);

                    Err(MergeError {
                        inner: e,
                        branch: self.branch,
                    })
                }
                Err(recovery_err) => {
                    self.branch.status = Some(BranchStatus::RecoveryFailed.to_string());

                    error!("Mere::Failed to recover: {}", recovery_err);
                    self.serialize_and_store_to_disk();

                    Err(MergeError {
                        inner: NodecosmosError::FatalMergeError(format!("Failed to merge and recover: {}", e)),
                        branch: self.branch,
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

    async fn merge(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        while self.merge_step < MergeStep::Finish {
            match self.merge_step {
                MergeStep::Start => (),
                MergeStep::RestoreNodes => self.nodes.restore_nodes(data, &self.branch).await?,
                MergeStep::CreateNodes => self.nodes.create_nodes(data, &self.branch).await?,
                MergeStep::DeleteNodes => self.nodes.delete_nodes(data).await?,
                MergeStep::ReorderNodes => self.nodes.reorder_nodes(data).await?,
                MergeStep::UpdateNodesTitles => self.nodes.update_title(data, &mut self.branch).await?,
                MergeStep::UpdateWorkflowInitialInputs => self.workflows.update_initial_inputs(data).await?,
                MergeStep::RestoreFlows => self.flows.restore_flows(data).await?,
                MergeStep::CreateFlows => self.flows.create_flows(data).await?,
                MergeStep::DeleteFlows => self.flows.delete_flows(data).await?,
                MergeStep::UpdateFlowsTitles => self.flows.update_title(data, &mut self.branch).await?,
                MergeStep::DeleteFlowSteps => self.flow_steps.delete_flow_steps(data).await?,
                MergeStep::RestoreFlowSteps => self.flow_steps.restore_flow_steps(data).await?,
                MergeStep::CreateFlowSteps => self.flow_steps.create_flow_steps(data, &mut self.branch).await?,
                MergeStep::CreateFlowStepNodes => self.flow_steps.create_flow_step_nodes(data).await?,
                MergeStep::DeleteFlowStepNodes => self.flow_steps.delete_flow_step_nodes(data, &self.branch).await?,
                MergeStep::CreateFlowStepInputs => self.flow_steps.create_inputs(data).await?,
                MergeStep::DeleteFlowStepInputs => self.flow_steps.delete_inputs(data, &self.branch).await?,
                MergeStep::CreateFlowStepOutputs => self.flow_steps.create_outputs(data).await?,
                MergeStep::DeleteFlowStepOutputs => self.flow_steps.delete_outputs(data, &self.branch).await?,
                MergeStep::RestoreIos => self.ios.restore_ios(data).await?,
                MergeStep::CreateIos => self.ios.create_ios(data).await?,
                MergeStep::DeleteIos => self.ios.delete_ios(data).await?,
                MergeStep::UpdateIoTitles => self.ios.update_title(data, &mut self.branch).await?,
                MergeStep::UpdateDescriptions => self.descriptions.update_descriptions(data, &mut self.branch).await?,
                MergeStep::DeleteDescriptions => self.descriptions.delete_descriptions(data).await?,
                MergeStep::Finish => (),
            }

            self.merge_step.increment();
        }

        Ok(())
    }

    /// Recover from merge failure in reverse order
    pub async fn recover(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        while self.merge_step > MergeStep::Start {
            match self.merge_step {
                MergeStep::Start => (),
                MergeStep::RestoreNodes => self.nodes.undo_delete_nodes(data).await?,
                MergeStep::CreateNodes => self.nodes.undo_create_nodes(data).await?,
                MergeStep::DeleteNodes => self.nodes.undo_restore_nodes(data).await?,
                MergeStep::ReorderNodes => self.nodes.undo_reorder_nodes(data).await?,
                MergeStep::UpdateNodesTitles => self.nodes.undo_update_title(data).await?,
                MergeStep::UpdateWorkflowInitialInputs => self.workflows.undo_update_initial_inputs(data).await?,
                MergeStep::RestoreFlows => self.flows.undo_create_flows(data).await?,
                MergeStep::CreateFlows => self.flows.undo_delete_flows(data).await?,
                MergeStep::DeleteFlows => self.flows.undo_restore_flows(data).await?,
                MergeStep::UpdateFlowsTitles => self.flows.undo_update_title(data).await?,
                MergeStep::DeleteFlowSteps => self.flow_steps.undo_restore_flow_steps(data).await?,
                MergeStep::RestoreFlowSteps => self.flow_steps.undo_create_flow_steps(data).await?,
                MergeStep::CreateFlowSteps => self.flow_steps.undo_delete_flow_steps(data).await?,
                MergeStep::CreateFlowStepNodes => self.flow_steps.undo_delete_flow_step_nodes(data).await?,
                MergeStep::DeleteFlowStepNodes => self.flow_steps.undo_create_flow_step_nodes(data).await?,
                MergeStep::CreateFlowStepInputs => self.flow_steps.undo_delete_inputs(data).await?,
                MergeStep::DeleteFlowStepInputs => self.flow_steps.undo_create_inputs(data).await?,
                MergeStep::CreateFlowStepOutputs => self.flow_steps.undo_delete_outputs(data).await?,
                MergeStep::DeleteFlowStepOutputs => self.flow_steps.undo_create_outputs(data).await?,
                MergeStep::RestoreIos => self.ios.undo_create_ios(data).await?,
                MergeStep::CreateIos => self.ios.undo_delete_ios(data).await?,
                MergeStep::DeleteIos => self.ios.undo_restore_ios(data).await?,
                MergeStep::UpdateIoTitles => self.ios.undo_update_title(data).await?,
                MergeStep::UpdateDescriptions => self.descriptions.undo_update_description(data).await?,
                MergeStep::DeleteDescriptions => self.descriptions.undo_delete_descriptions(data).await?,
                MergeStep::Finish => (),
            }

            self.merge_step.decrement();
        }

        Ok(())
    }

    async fn unlock_resource(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        let node = self.branch.node(data.db_session()).await?;

        data.resource_locker()
            .unlock_resource(node.root_id, node.branch_id)
            .await?;
        data.resource_locker()
            .unlock_resource_action(ActionTypes::Merge, node.root_id, node.branch_id)
            .await?;

        Ok(())
    }

    fn serialize_and_store_to_disk(&self) {
        // serialize branch_merge and store to disk
        let serialized = serde_json::to_string(self).expect("Failed to serialize branch merge data");
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

        let merge = BranchMerge::new(data, self)
            .await?
            .check_conflicts(data)
            .await?
            .run(data)
            .await?;

        let res = merge.branch.update().execute(data.db_session()).await;

        if let Err(e) = res {
            return Err(MergeError {
                inner: NodecosmosError::from(e),
                branch: merge.branch,
            });
        }

        Ok(merge.branch)
    }

    pub async fn validate_no_existing_conflicts(&mut self) -> Result<(), NodecosmosError> {
        if self.conflict.is_some() {
            return Err(NodecosmosError::Conflict("Conflicts not resolved".to_string()));
        }

        Ok(())
    }
    pub async fn check_conflicts(self, data: &RequestData) -> Result<Self, MergeError> {
        let merge = BranchMerge::new(data, self).await?.check_conflicts(data).await?;

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
