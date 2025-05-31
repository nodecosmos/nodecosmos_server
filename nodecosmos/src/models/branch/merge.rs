use charybdis::operations::Update;
use charybdis::types::Uuid;
use log::{error, warn};
use scylla::client::caching_session::CachingSession;
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
use crate::models::branch::{Branch, BranchStatus};
use crate::models::recovery::{RecoveryLog, RecoveryObjectType};

mod conflicts;
mod descriptions;
mod flow_steps;
mod flows;
mod ios;
mod nodes;

#[derive(Debug)]
pub struct MergeError {
    pub inner: NodecosmosError,
    pub branch: Branch,
}

#[derive(Clone, Copy, Serialize, Deserialize, PartialOrd, PartialEq, Debug)]
pub enum MergeStep {
    BeforeStart = -1,
    Start = 0,
    RestoreNodes = 1,
    CreateNodes = 2,
    DeleteNodes = 3,
    UpdateNodesTitles = 4,
    ReorderNodes = 5,
    RestoreFlows = 6,
    CreateFlows = 7,
    DeleteFlows = 8,
    UpdateFlowsTitles = 9,
    DeleteFlowSteps = 10,
    RestoreFlowSteps = 11,
    CreateFlowSteps = 12,
    CreateFlowStepNodes = 13,
    DeleteFlowStepNodes = 14,
    CreateFlowStepInputs = 15,
    DeleteFlowStepInputs = 16,
    RestoreIos = 17,
    CreateIos = 18,
    DeleteIos = 19,
    UpdateIoTitles = 20,
    UpdateDescriptions = 21,
    DeleteDescriptions = 22,
    Finish = 23,
    AfterFinish = 24,
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
            -1 => MergeStep::BeforeStart,
            0 => MergeStep::Start,
            1 => MergeStep::RestoreNodes,
            2 => MergeStep::CreateNodes,
            3 => MergeStep::DeleteNodes,
            4 => MergeStep::UpdateNodesTitles,
            5 => MergeStep::ReorderNodes,
            6 => MergeStep::RestoreFlows,
            7 => MergeStep::CreateFlows,
            8 => MergeStep::DeleteFlows,
            9 => MergeStep::UpdateFlowsTitles,
            10 => MergeStep::DeleteFlowSteps,
            11 => MergeStep::RestoreFlowSteps,
            12 => MergeStep::CreateFlowSteps,
            13 => MergeStep::CreateFlowStepNodes,
            14 => MergeStep::DeleteFlowStepNodes,
            15 => MergeStep::CreateFlowStepInputs,
            16 => MergeStep::DeleteFlowStepInputs,
            17 => MergeStep::RestoreIos,
            18 => MergeStep::CreateIos,
            19 => MergeStep::DeleteIos,
            20 => MergeStep::UpdateIoTitles,
            21 => MergeStep::UpdateDescriptions,
            22 => MergeStep::DeleteDescriptions,
            23 => MergeStep::Finish,
            24 => MergeStep::AfterFinish,
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
                MergeStep::BeforeStart => {
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
                MergeStep::RestoreFlows => self.flows.restore_flows(data).await?,
                MergeStep::CreateFlows => self.flows.create_flows(data).await?,
                MergeStep::DeleteFlows => self.flows.delete_flows(data).await?,
                MergeStep::UpdateFlowsTitles => self.flows.update_title(data, &mut self.branch).await?,
                MergeStep::DeleteFlowSteps => self.flow_steps.delete_flow_steps(data).await?,
                MergeStep::RestoreFlowSteps => self.flow_steps.restore_flow_steps(data).await?,
                MergeStep::CreateFlowSteps => self.flow_steps.create_flow_steps(data, &self.branch).await?,
                MergeStep::CreateFlowStepNodes => self.flow_steps.create_flow_step_nodes(data).await?,
                MergeStep::DeleteFlowStepNodes => self.flow_steps.delete_flow_step_nodes(data, &self.branch).await?,
                MergeStep::CreateFlowStepInputs => self.flow_steps.create_inputs(data).await?,
                MergeStep::DeleteFlowStepInputs => self.flow_steps.delete_inputs(data, &self.branch).await?,
                MergeStep::RestoreIos => self.ios.restore_ios(data).await?,
                MergeStep::CreateIos => self.ios.create_ios(data).await?,
                MergeStep::DeleteIos => self.ios.delete_ios(data).await?,
                MergeStep::UpdateIoTitles => self.ios.update_title(data, &mut self.branch).await?,
                MergeStep::UpdateDescriptions => self.descriptions.update_descriptions(data, &mut self.branch).await?,
                MergeStep::DeleteDescriptions => self.descriptions.delete_descriptions(data).await?,
                MergeStep::Finish => {
                    self.delete_recovery_log(data.db_session()).await?;
                }
                MergeStep::AfterFinish => {
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
                MergeStep::BeforeStart => {
                    log::error!("should not hit before placeholder");
                }
                MergeStep::Start => {
                    self.delete_recovery_log(data.db_session()).await?;
                }
                MergeStep::RestoreNodes => self.nodes.undo_restore_nodes(data).await?,
                MergeStep::CreateNodes => self.nodes.undo_create_nodes(data).await?,
                MergeStep::DeleteNodes => self.nodes.undo_delete_nodes(data).await?,
                MergeStep::UpdateNodesTitles => self.nodes.undo_update_title(data).await?,
                MergeStep::ReorderNodes => self.nodes.undo_reorder_nodes(data, &self.branch).await?,
                MergeStep::RestoreFlows => self.flows.undo_restore_flows(data).await?,
                MergeStep::CreateFlows => self.flows.undo_create_flows(data).await?,
                MergeStep::DeleteFlows => self.flows.undo_delete_flows(data).await?,
                MergeStep::UpdateFlowsTitles => self.flows.undo_update_title(data).await?,
                MergeStep::DeleteFlowSteps => self.flow_steps.undo_delete_flow_steps(data).await?,
                MergeStep::RestoreFlowSteps => self.flow_steps.undo_restore_flow_steps(data).await?,
                MergeStep::CreateFlowSteps => self.flow_steps.undo_create_flow_steps(data).await?,
                MergeStep::CreateFlowStepNodes => self.flow_steps.undo_create_flow_step_nodes(data).await?,
                MergeStep::DeleteFlowStepNodes => self.flow_steps.undo_delete_flow_step_nodes(data).await?,
                MergeStep::CreateFlowStepInputs => self.flow_steps.undo_create_inputs(data).await?,
                MergeStep::DeleteFlowStepInputs => self.flow_steps.undo_delete_inputs(data).await?,
                MergeStep::RestoreIos => self.ios.undo_restore_ios(data).await?,
                MergeStep::CreateIos => self.ios.undo_create_ios(data).await?,
                MergeStep::DeleteIos => self.ios.undo_delete_ios(data).await?,
                MergeStep::UpdateIoTitles => self.ios.undo_update_title(data).await?,
                MergeStep::UpdateDescriptions => self.descriptions.undo_update_description(data).await?,
                MergeStep::DeleteDescriptions => self.descriptions.undo_delete_descriptions(data).await?,
                MergeStep::Finish => {
                    log::error!("should not recover finished process");
                }
                MergeStep::AfterFinish => {
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

    fn set_step(&mut self, step: i8) {
        self.merge_step = MergeStep::from(step);
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
    pub async fn merge(self, data: &RequestData) -> Result<Self, MergeError> {
        // There are scenarios where we want to check for conflicts again before merging,
        // and this will disallow merging even in the case of resolved conflicts.
        // if let Err(e) = self.validate_no_existing_conflicts().await {
        //     return Err(MergeError { inner: e, branch: self });
        // }

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

    #[allow(unused)]
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::branch::update::BranchUpdate;
    use crate::models::contribution_request::ContributionRequest;
    use crate::models::description::Description;
    use crate::models::flow::Flow;
    use crate::models::flow_step::FlowStep;
    use crate::models::io::Io;
    use crate::models::node::Node;
    use crate::models::node_descendant::NodeDescendant;
    use crate::models::traits::{Descendants, NodeBranchParams, Reload};
    use charybdis::operations::{DeleteWithCallbacks, UpdateWithCallbacks};

    pub struct TestMerge {
        pub data: RequestData,
        pub branch: Branch,
        pub root: Node,
        pub branch_node: Node,
        pub branch_id: uuid::Uuid,
        pub original_params: NodeBranchParams,
        pub branch_params: NodeBranchParams,
    }

    #[allow(unused)]
    impl TestMerge {
        async fn new() -> Self {
            let data = RequestData::new(None).await;
            let root = Node::sample_node_tree(&data).await;
            let root_id = root.root_id;
            let mut cr = ContributionRequest::create_test_cr(&data, &root).await;
            let branch_id = cr.id;
            let branch = cr
                .branch(data.db_session())
                .await
                .expect("Failed to get branch")
                .clone();
            let branch_node = Node::find_by_branch_id_and_id(branch_id, root_id)
                .execute(data.db_session())
                .await
                .expect("Failed to find branch node");

            Self {
                data,
                branch,
                root,
                branch_node,
                branch_id,
                original_params: NodeBranchParams {
                    root_id,
                    branch_id: root_id,
                    node_id: root_id,
                },
                branch_params: NodeBranchParams {
                    root_id,
                    branch_id,
                    node_id: root_id,
                },
            }
        }

        async fn original_flows(&self) -> Vec<Flow> {
            Flow::find_by_branch_id(self.root.root_id)
                .execute(self.data.db_session())
                .await
                .expect("Failed to find flows")
                .try_collect()
                .await
                .expect("Failed to collect flows")
        }

        async fn branch_flows(&self) -> Vec<Flow> {
            Flow::find_by_branch_id(self.branch_id)
                .execute(self.data.db_session())
                .await
                .expect("Failed to find flows")
                .try_collect()
                .await
                .expect("Failed to collect flows")
        }

        async fn original_flow_steps(&self) -> Vec<FlowStep> {
            FlowStep::find_by_branch_id(self.root.root_id)
                .execute(self.data.db_session())
                .await
                .expect("Failed to find flow steps")
                .try_collect()
                .await
                .expect("Failed to collect flow steps")
        }

        async fn branch_flow_steps(&self) -> Vec<FlowStep> {
            FlowStep::find_by_branch_id(self.branch_id)
                .execute(self.data.db_session())
                .await
                .expect("Failed to find flow steps")
                .try_collect()
                .await
                .expect("Failed to collect flow steps")
        }

        async fn original_ios(&self) -> Vec<Io> {
            Io::find_by_branch_id(self.root.root_id)
                .execute(self.data.db_session())
                .await
                .expect("Failed to find IOs")
                .try_collect()
                .await
                .expect("Failed to collect IOs")
        }

        async fn branch_ios(&self) -> Vec<Io> {
            Io::find_by_branch_id(self.branch_id)
                .execute(self.data.db_session())
                .await
                .expect("Failed to find IOs")
                .try_collect()
                .await
                .expect("Failed to collect IOs")
        }

        async fn original_descendants(&self) -> Vec<NodeDescendant> {
            self.root
                .descendants(self.data.db_session())
                .await
                .expect("Failed to get descendants")
                .try_collect()
                .await
                .expect("Failed to collect descendants")
        }

        async fn branch_descendants(&self) -> Vec<NodeDescendant> {
            self.branch_node
                .branch_descendants(self.data.db_session())
                .await
                .expect("Failed to collect branch descendants")
        }
    }

    #[tokio::test]
    async fn test_merge_with_no_conflicts() {
        let mut test_merge = TestMerge::new().await;
        test_merge
            .root
            .create_branched_nodes_for_each_descendant(&test_merge.data, test_merge.branch_id)
            .await
            .unwrap();
        let original_descendants = test_merge.original_descendants().await;

        // Create an original Flow
        let original_flow = Flow::create_test_flow(&test_merge.data, &test_merge.original_params).await;

        // Create a branch flow step for the original flow
        FlowStep::create_test_flow_step(&test_merge.data, &test_merge.branch_params, original_flow.id).await;

        // Create a branch Flow for root
        let branch_flow = Flow::create_test_flow(&test_merge.data, &test_merge.branch_params).await;

        Flow::create_test_node_flows(
            &test_merge.data,
            &test_merge.branch_params,
            original_descendants.iter().take(10),
        )
        .await;

        // Create a branch FlowStep for the branch flow
        let branch_flow_step =
            FlowStep::create_test_flow_step(&test_merge.data, &test_merge.branch_params, branch_flow.id).await;

        // Create an branch IO for the branch flow step
        Io::create_test_io(&test_merge.data, &test_merge.branch_params, Some(branch_flow_step.id)).await;

        assert_eq!(
            original_descendants.len(),
            110,
            "Original node descendants count should be 110"
        );
        assert_eq!(
            test_merge.branch_descendants().await.len(),
            220,
            "Branch node descendants count should be 220 (original + branched)"
        );

        test_merge.branch.reload(test_merge.data.db_session()).await.unwrap();

        test_merge.branch = test_merge.branch.merge(&test_merge.data).await.unwrap();

        assert_eq!(
            test_merge.original_descendants().await.len(),
            220,
            "After merge, original node descendants count should be 220"
        );

        assert_eq!(
            test_merge.original_flows().await.len(),
            12,
            "After merge, there should be 12 flows (1 original + 1 branch root + 10 branch descendants)"
        );

        assert_eq!(
            test_merge.original_flow_steps().await.len(),
            2,
            "After merge, there should be 2 flow steps (original + branched)"
        );

        assert_eq!(
            test_merge.original_ios().await.len(),
            1,
            "After merge, there should be 1 IO"
        );
    }

    #[tokio::test]
    async fn test_merge_with_deleted_edited_nodes_conflict() {
        // Create test environment
        let mut test_merge = TestMerge::new().await;

        // Get original descendants
        let original_descendants = test_merge.original_descendants().await;
        let last_descendant = original_descendants.last().unwrap();

        // Delete a node in the original branch
        let mut node_to_delete = Node::find_by_branch_id_and_id(test_merge.root.root_id, last_descendant.id)
            .execute(test_merge.data.db_session())
            .await
            .expect("Failed to find node to delete");

        // edit node that will be deleted
        Description::create_test_description(
            &test_merge.data,
            test_merge.branch_id,
            node_to_delete.root_id,
            node_to_delete.id,
            node_to_delete.id,
        )
        .await;

        test_merge.branch.reload(test_merge.data.db_session()).await.unwrap();

        // Delete the node in the original branch
        node_to_delete
            .delete_cb(&test_merge.data)
            .execute(test_merge.data.db_session())
            .await
            .expect("Failed to delete node");

        // Check for conflicts
        let mut branch_clone = test_merge.branch.clone();
        test_merge.branch.reload(test_merge.data.db_session()).await.unwrap();
        let merge_result = test_merge.branch.merge(&test_merge.data).await;

        assert!(merge_result.is_err(), "Expected the conflict error but got Ok");

        branch_clone.reload(test_merge.data.db_session()).await.unwrap();
        let conflict = branch_clone.conflict.expect("Branch should have conflicts");

        assert!(
            conflict.deleted_edited_nodes.is_some(),
            "Branch should have deleted_edited_nodes conflict"
        );

        assert!(
            conflict
                .deleted_edited_nodes
                .is_some_and(|ids| ids.contains(&last_descendant.id)),
            "Conflict should contain the deleted and edited node"
        );
    }

    #[tokio::test]
    async fn test_merge_with_conflicting_flow_steps() {
        // Create test environment
        let mut test_merge = TestMerge::new().await;

        // Create branched nodes for each descendant
        test_merge
            .root
            .create_branched_nodes_for_each_descendant(&test_merge.data, test_merge.branch_id)
            .await
            .unwrap();

        // Create an original Flow
        let original_flow = Flow::create_test_flow(&test_merge.data, &test_merge.original_params).await;

        // Create a flow step for the original flow with a specific step_index
        let mut original_flow_step =
            FlowStep::create_test_flow_step(&test_merge.data, &test_merge.original_params, original_flow.id).await;
        original_flow_step.step_index = 1.into();
        original_flow_step
            .update_cb(&test_merge.data)
            .execute(test_merge.data.db_session())
            .await
            .expect("Failed to update original flow step");

        // Create a flow step for the same flow in the branch with the same step_index
        let mut branch_flow_step =
            FlowStep::create_test_flow_step(&test_merge.data, &test_merge.branch_params, original_flow.id).await;
        branch_flow_step.step_index = 1.into();
        branch_flow_step
            .update_cb(&test_merge.data)
            .execute(test_merge.data.db_session())
            .await
            .expect("Failed to update branch flow step");

        // Check for conflicts
        test_merge.branch.reload(test_merge.data.db_session()).await.unwrap();
        let check_conflict_res = test_merge
            .branch
            .clone()
            .check_conflicts(test_merge.data.db_session())
            .await;

        assert!(check_conflict_res.is_err(), "check_conflict_res should return an error");

        test_merge.branch.reload(test_merge.data.db_session()).await.unwrap();

        let conflict = test_merge
            .branch
            .conflict
            .clone()
            .expect("Branch should have conflicts");
        assert!(
            conflict.conflicting_flow_steps.is_some(),
            "Branch should have conflicting_flow_steps conflict"
        );

        assert!(
            conflict
                .conflicting_flow_steps
                .is_some_and(|ids| ids.contains(&branch_flow_step.id)),
            "Conflict should contain the conflicting flow step"
        );
    }

    #[tokio::test]
    async fn test_merge_recovery_from_conflicts() {
        // Create test environment
        let mut test_merge = TestMerge::new().await;

        // Get original descendants
        let original_descendants = test_merge.original_descendants().await;
        let last_descendant = original_descendants.first().unwrap();

        // Find the node to delete in the original branch
        let mut node_to_delete = Node::find_by_branch_id_and_id(test_merge.root.root_id, last_descendant.id)
            .execute(test_merge.data.db_session())
            .await
            .expect("Failed to find node to delete");

        // Edit the same node in the branch by creating a description
        Description::create_test_description(
            &test_merge.data,
            test_merge.branch_id,
            node_to_delete.root_id,
            node_to_delete.id,
            node_to_delete.id,
        )
        .await;

        // Delete a node in the original branch
        node_to_delete
            .delete_cb(&test_merge.data)
            .execute(test_merge.data.db_session())
            .await
            .expect("Failed to delete node");

        // Check for conflicts
        test_merge.branch.reload(test_merge.data.db_session()).await.unwrap();
        let check_conflict_res = test_merge
            .branch
            .clone()
            .check_conflicts(test_merge.data.db_session())
            .await;

        assert!(check_conflict_res.is_err(), "check_conflict_res should return an error");

        test_merge.branch.reload(test_merge.data.db_session()).await.unwrap();

        // Verify that the conflict is correctly identified
        assert!(test_merge.branch.conflict.is_some(), "Branch should have conflicts");

        // Clear the conflicts to simulate resolving them
        // restore the deleted node
        Branch::update(
            test_merge.data.db_session(),
            test_merge.branch_id,
            BranchUpdate::RestoreNode(node_to_delete.id),
        )
        .await
        .unwrap();

        // Attempt to merge again
        test_merge.branch.reload(test_merge.data.db_session()).await.unwrap();
        let merged_branch = test_merge.branch.merge(&test_merge.data).await.unwrap();

        // Verify that the merge was successful
        assert_eq!(
            merged_branch.status,
            Some("Merged".into()),
            "Branch status should be Merged"
        );
    }
}
