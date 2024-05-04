use charybdis::operations::Update;
use charybdis::types::Uuid;
use log::{error, warn};
use scylla::CachingSession;
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
use crate::models::recovery::{RecoveryLog, RecoveryObjectType};

mod conflicts;
mod descriptions;
mod flow_steps;
mod flows;
mod ios;
mod nodes;
mod workflows;

pub struct MergeError {
    pub inner: NodecosmosError,
    pub branch: Branch,
}

#[derive(Clone, Copy, Serialize, Deserialize, PartialOrd, PartialEq, Debug)]
pub enum MergeStep {
    BeforePlaceholder = -1,
    Start = 0,
    RestoreNodes = 1,
    CreateNodes = 2,
    DeleteNodes = 3,
    UpdateNodesTitles = 4,
    ReorderNodes = 5,
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
    AfterPlaceholder = 27,
}

impl MergeStep {
    pub fn increment(&mut self) {
        *self = MergeStep::from(*self as i8 + 1);
    }

    pub fn decrement(&mut self) {
        *self = MergeStep::from(*self as i8 - 1);
    }
}

impl From<i8> for MergeStep {
    fn from(value: i8) -> Self {
        match value {
            -1 => MergeStep::BeforePlaceholder,
            0 => MergeStep::Start,
            1 => MergeStep::RestoreNodes,
            2 => MergeStep::CreateNodes,
            3 => MergeStep::DeleteNodes,
            4 => MergeStep::UpdateNodesTitles,
            5 => MergeStep::ReorderNodes,
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
            27 => MergeStep::AfterPlaceholder,
            _ => panic!("Invalid merge step value: {}", value),
        }
    }
}

/// ScyllaDB does not support transactions like traditional databases.
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
    pub async fn new(db_session: &CachingSession, branch: Branch) -> Result<Self, MergeError> {
        let nodes = MergeNodes::new(db_session, &branch).await.map_err(|e| MergeError {
            inner: e,
            branch: branch.clone(),
        })?;

        let workflows = MergeWorkflows::new(&branch);

        let ios = MergeIos::new(db_session, &branch).await.map_err(|e| MergeError {
            inner: e,
            branch: branch.clone(),
        })?;

        let flows = MergeFlows::new(db_session, &branch).await.map_err(|e| MergeError {
            inner: e,
            branch: branch.clone(),
        })?;

        let flow_steps = MergeFlowSteps::new(db_session, &branch).await.map_err(|e| MergeError {
            inner: e,
            branch: branch.clone(),
        })?;

        let descriptions = MergeDescriptions::new(db_session, &branch)
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

    pub async fn check_conflicts(mut self, db_session: &CachingSession) -> Result<Self, MergeError> {
        if let Err(e) = MergeConflicts::new(&mut self).run_check(db_session).await {
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

                    Err(MergeError {
                        inner: NodecosmosError::FatalMergeError(format!("Failed to merge and recover: {}", e)),
                        branch: self.branch,
                    })
                }
            },
        }
    }

    async fn merge(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        while self.merge_step <= MergeStep::Finish {
            // log current step
            if self.merge_step > MergeStep::Start && self.merge_step < MergeStep::Finish {
                self.update_recovery_log_step(data.db_session(), self.merge_step as i8)
                    .await?;
            }

            match self.merge_step {
                MergeStep::BeforePlaceholder => {
                    log::error!("should not hit before placeholder");
                }
                MergeStep::Start => {
                    self.create_recovery_log(data.db_session()).await?;
                }
                MergeStep::RestoreNodes => self.nodes.restore_nodes(data, &mut self.branch).await?,
                MergeStep::CreateNodes => self.nodes.create_nodes(data, &mut self.branch).await?,
                MergeStep::DeleteNodes => self.nodes.delete_nodes(data).await?,
                MergeStep::UpdateNodesTitles => self.nodes.update_title(data, &mut self.branch).await?,
                MergeStep::ReorderNodes => self.nodes.reorder_nodes(data, &self.branch).await?,
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
                MergeStep::Finish => {
                    self.delete_recovery_log(data.db_session()).await?;
                }
                MergeStep::AfterPlaceholder => {
                    log::error!("should not hit after placeholder");
                }
            }

            self.merge_step.increment();
        }

        Ok(())
    }

    /// Recover from merge failure in reverse order
    pub async fn recover(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        while self.merge_step >= MergeStep::Start {
            // log current step
            if self.merge_step < MergeStep::Finish && self.merge_step > MergeStep::Start {
                self.update_recovery_log_step(data.db_session(), self.merge_step as i8)
                    .await?;
            }

            match self.merge_step {
                MergeStep::BeforePlaceholder => {
                    log::error!("should not hit before placeholder");
                }
                MergeStep::Start => {
                    self.delete_recovery_log(data.db_session()).await?;
                }
                MergeStep::RestoreNodes => self.nodes.undo_delete_nodes(data).await?,
                MergeStep::CreateNodes => self.nodes.undo_create_nodes(data).await?,
                MergeStep::DeleteNodes => self.nodes.undo_restore_nodes(data).await?,
                MergeStep::UpdateNodesTitles => self.nodes.undo_update_title(data).await?,
                MergeStep::ReorderNodes => self.nodes.undo_reorder_nodes(data, &self.branch).await?,
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
                MergeStep::Finish => {
                    log::error!("should not recover finished process");
                }
                MergeStep::AfterPlaceholder => {
                    log::error!("should not hit after placeholder");
                }
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
}

impl RecoveryLog<'_> for BranchMerge {
    fn rec_id(&self) -> Uuid {
        self.branch.id
    }

    fn rec_branch_id(&self) -> Uuid {
        self.branch.id
    }

    fn rec_object_type(&self) -> RecoveryObjectType {
        RecoveryObjectType::Merge
    }

    async fn recover_from_log(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        self.recover(data).await.map_err(|e| {
            log::error!(
                "Fatal Merge Error: recover_from_log failed for branch: {}\n! ERROR: {:?}",
                self.branch.id,
                e
            );
            NodecosmosError::FatalDeleteError(format!("Error recovering from log: {:?}", e))
        })?;

        let _ = self.unlock_resource(data).await.map_err(|e| {
            log::error!(
                "Merge Error: unlock_resource failed for branch: {}\n! ERROR: {:?}",
                self.branch.id,
                e
            );

            NodecosmosError::FatalDeleteError(format!("Error unlocking resource: {:?}", e))
        });

        Ok(())
    }
}

impl Branch {
    pub async fn merge(mut self, data: &RequestData) -> Result<Self, MergeError> {
        if let Err(e) = self.validate_no_existing_conflicts().await {
            return Err(MergeError { inner: e, branch: self });
        }

        let merge = BranchMerge::new(data.db_session(), self)
            .await?
            .check_conflicts(data.db_session())
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
    pub async fn check_conflicts(self, db_session: &CachingSession) -> Result<Self, MergeError> {
        let merge = BranchMerge::new(db_session, self)
            .await?
            .check_conflicts(db_session)
            .await?;

        let res = merge.branch.update().execute(db_session).await;

        if let Err(e) = res {
            return Err(MergeError {
                inner: NodecosmosError::from(e),
                branch: merge.branch,
            });
        }

        Ok(merge.branch)
    }
}
