use crate::models::branch::{
    Branch, UpdateCreatedFlowStepInputsByNodeBranch, UpdateCreatedFlowStepOutputsByNodeBranch,
    UpdateCreatedFlowStepsBranch, UpdateCreatedFlowsBranch, UpdateCreatedIOsBranch, UpdateCreatedNodesBranch,
    UpdateCreatedWorkflowsBranch, UpdateDeletedFlowStepInputsByNodeBranch, UpdateDeletedFlowStepOutputsByNodeBranch,
    UpdateDeletedFlowStepsBranch, UpdateDeletedFlowsBranch, UpdateDeletedIOsBranch, UpdateDeletedNodesBranch,
    UpdateDeletedWorkflowsBranch, UpdateEditedFlowDescriptionsBranch, UpdateEditedFlowTitlesBranch,
    UpdateEditedIODescriptionsBranch, UpdateEditedIOTitlesBranch, UpdateEditedNodeDescriptionsBranch,
    UpdateEditedNodeTitlesBranch, UpdateEditedNodeTreePositionIdBranch, UpdateEditedWorkflowTitlesBranch,
};
use crate::utils::logger::log_fatal;
use charybdis::errors::CharybdisError;
use charybdis::operations::{Find, Update};
use charybdis::types::{Set, Uuid};
use scylla::{CachingSession, QueryResult};

pub enum BranchUpdate {
    CreateNode(Uuid),
    DeleteNode(Uuid),
    EditNodeTitle(Uuid),
    EditNodeDescription(Uuid),
    EditPosition(Uuid),
    CreateWorkflow(Uuid),
    DeleteWorkflow(Uuid),
    EditWorkflowTitle(Uuid),
    CreateFlow(Uuid),
    DeleteFlow(Uuid),
    EditFlowTitle(Uuid),
    EditFlowDescription(Uuid),
    CreateIO(Uuid),
    DeleteIO(Uuid),
    EditIOTitle(Uuid),
    EditIODescription(Uuid),
    CreateFlowStep(Uuid),
    DeleteFlowStep(Uuid),
    AppendFlowStepInput(Uuid, Uuid),
    RemoveFlowStepInput(Uuid, Uuid),
    AppendFlowStepOutput(Uuid, Uuid),
    RemoveFlowStepOutput(Uuid, Uuid),
}

impl Branch {
    pub async fn update(session: &CachingSession, branch_id: Uuid, update: BranchUpdate) {
        let res: Result<QueryResult, CharybdisError>;

        match update {
            BranchUpdate::CreateNode(id) => {
                res = UpdateCreatedNodesBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .push_created_nodes(session, &vec![id])
                .await;
            }
            BranchUpdate::DeleteNode(id) => {
                res = UpdateDeletedNodesBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .push_deleted_nodes(session, &vec![id])
                .await;
            }
            BranchUpdate::EditNodeTitle(id) => {
                res = UpdateEditedNodeTitlesBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .push_edited_node_titles(session, &vec![id])
                .await;
            }
            BranchUpdate::EditNodeDescription(id) => {
                res = UpdateEditedNodeDescriptionsBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .push_edited_node_descriptions(session, &vec![id])
                .await;
            }
            BranchUpdate::EditPosition(id) => {
                res = UpdateEditedNodeTreePositionIdBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .push_edited_node_tree_positions(session, &vec![id])
                .await;
            }
            BranchUpdate::CreateWorkflow(id) => {
                res = UpdateCreatedWorkflowsBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .push_created_workflows(session, &vec![id])
                .await;
            }
            BranchUpdate::DeleteWorkflow(id) => {
                res = UpdateDeletedWorkflowsBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .push_deleted_workflows(session, &vec![id])
                .await;
            }
            BranchUpdate::EditWorkflowTitle(id) => {
                res = UpdateEditedWorkflowTitlesBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .push_edited_workflow_titles(session, &vec![id])
                .await;
            }
            BranchUpdate::CreateFlow(id) => {
                res = UpdateCreatedFlowsBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .push_created_flows(session, &vec![id])
                .await;
            }
            BranchUpdate::DeleteFlow(id) => {
                res = UpdateDeletedFlowsBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .push_deleted_flows(session, &vec![id])
                .await;
            }
            BranchUpdate::EditFlowTitle(id) => {
                res = UpdateEditedFlowTitlesBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .push_edited_flow_titles(session, &vec![id])
                .await;
            }
            BranchUpdate::EditFlowDescription(id) => {
                res = UpdateEditedFlowDescriptionsBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .push_edited_flow_descriptions(session, &vec![id])
                .await;
            }
            BranchUpdate::CreateIO(id) => {
                res = UpdateCreatedIOsBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .push_created_ios(session, &vec![id])
                .await;
            }
            BranchUpdate::DeleteIO(id) => {
                res = UpdateDeletedIOsBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .push_deleted_ios(session, &vec![id])
                .await;
            }
            BranchUpdate::EditIOTitle(id) => {
                res = UpdateEditedIOTitlesBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .push_edited_io_titles(session, &vec![id])
                .await;
            }
            BranchUpdate::EditIODescription(id) => {
                res = UpdateEditedIODescriptionsBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .push_edited_io_descriptions(session, &vec![id])
                .await;
            }
            BranchUpdate::CreateFlowStep(id) => {
                res = UpdateCreatedFlowStepsBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .push_created_flow_steps(session, &vec![id])
                .await;
            }
            BranchUpdate::DeleteFlowStep(id) => {
                res = UpdateDeletedFlowStepsBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .push_deleted_flow_steps(session, &vec![id])
                .await;
            }
            BranchUpdate::AppendFlowStepInput(node_id, io_id) => {
                let response = UpdateCreatedFlowStepInputsByNodeBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .find_by_primary_key(session)
                .await;

                match response {
                    Ok(mut fs_branch) => {
                        let _ = &mut fs_branch
                            .created_flow_step_inputs_by_node
                            .get_or_insert_with(Default::default)
                            .entry(node_id)
                            .or_insert_with(Set::default)
                            .insert(io_id);

                        res = fs_branch.update(session).await;
                    }
                    Err(err) => {
                        log_fatal(format!("Failed to find branch: {}", err));
                        return;
                    }
                }
            }
            BranchUpdate::RemoveFlowStepInput(node_id, io_id) => {
                let response = UpdateDeletedFlowStepInputsByNodeBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .find_by_primary_key(session)
                .await;

                match response {
                    Ok(mut fs_branch) => {
                        let _ = &mut fs_branch
                            .deleted_flow_step_inputs_by_node
                            .get_or_insert_with(Default::default)
                            .entry(node_id)
                            .or_insert_with(Set::default)
                            .insert(io_id);

                        res = fs_branch.update(session).await;
                    }
                    Err(err) => {
                        log_fatal(format!("Failed to find branch: {}", err));
                        return;
                    }
                }
            }
            BranchUpdate::AppendFlowStepOutput(node_id, io_id) => {
                let response = UpdateCreatedFlowStepOutputsByNodeBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .find_by_primary_key(session)
                .await;

                match response {
                    Ok(mut fs_branch) => {
                        let _ = &mut fs_branch
                            .created_flow_step_outputs_by_node
                            .get_or_insert_with(Default::default)
                            .entry(node_id)
                            .or_insert_with(Set::default)
                            .insert(io_id);

                        res = fs_branch.update(session).await;
                    }
                    Err(err) => {
                        log_fatal(format!("Failed to find branch: {}", err));
                        return;
                    }
                }
            }
            BranchUpdate::RemoveFlowStepOutput(node_id, io_id) => {
                let response = UpdateDeletedFlowStepOutputsByNodeBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .find_by_primary_key(session)
                .await;

                match response {
                    Ok(mut fs_branch) => {
                        let _ = &mut fs_branch
                            .deleted_flow_step_outputs_by_node
                            .get_or_insert_with(Default::default)
                            .entry(node_id)
                            .or_insert_with(Set::default)
                            .insert(io_id);

                        res = fs_branch.update(session).await;
                    }
                    Err(err) => {
                        log_fatal(format!("Failed to find branch: {}", err));
                        return;
                    }
                }
            }
        }

        if let Err(err) = res {
            log_fatal(format!("Failed to update branch: {}", err));
        }
    }
}
