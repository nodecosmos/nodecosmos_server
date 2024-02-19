use crate::errors::NodecosmosError;
use crate::models::branch::{
    Branch, UpdateCreatedFlowStepInputsByNodeBranch, UpdateCreatedFlowStepOutputsByNodeBranch,
    UpdateCreatedFlowStepsBranch, UpdateCreatedFlowsBranch, UpdateCreatedIOsBranch, UpdateCreatedNodesBranch,
    UpdateCreatedWorkflowsBranch, UpdateDeletedFlowStepInputsByNodeBranch, UpdateDeletedFlowStepOutputsByNodeBranch,
    UpdateDeletedFlowStepsBranch, UpdateDeletedFlowsBranch, UpdateDeletedIOsBranch, UpdateDeletedNodesBranch,
    UpdateDeletedWorkflowsBranch, UpdateEditedFlowDescriptionsBranch, UpdateEditedFlowTitlesBranch,
    UpdateEditedIODescriptionsBranch, UpdateEditedIOTitlesBranch, UpdateEditedNodeDescriptionsBranch,
    UpdateEditedNodeTitlesBranch, UpdateEditedWorkflowTitlesBranch, UpdateReorderedNodes, UpdateRestoredNodesBranch,
};
use crate::models::udts::{BranchReorderData, TextChange};
use crate::models::utils::append_statement_or_log_fatal;
use crate::utils::logger::log_fatal;
use charybdis::batch::CharybdisModelBatch;
use charybdis::errors::CharybdisError;
use charybdis::operations::{Find, Update};
use charybdis::types::{Map, Set, Uuid};
use scylla::{CachingSession, QueryResult};

pub enum BranchUpdate {
    CreateNode(Uuid),
    DeleteNode(Uuid),
    RestoreNode(Uuid),
    EditNodeTitle(Uuid),
    EditNodeDescription(Uuid),
    ReorderNode(BranchReorderData),
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
                .push_created_nodes(&vec![id])
                .execute(session)
                .await;
            }
            BranchUpdate::DeleteNode(id) => {
                let mut batch = CharybdisModelBatch::new();
                let params = (vec![id], branch_id);

                if append_statement_or_log_fatal(
                    &mut batch,
                    UpdateDeletedNodesBranch::PUSH_DELETED_NODES_QUERY,
                    params.clone(),
                )
                .is_err()
                {
                    return;
                }

                if append_statement_or_log_fatal(
                    &mut batch,
                    UpdateCreatedNodesBranch::PULL_CREATED_NODES_QUERY,
                    params.clone(),
                )
                .is_err()
                {
                    return;
                }

                if append_statement_or_log_fatal(
                    &mut batch,
                    UpdateRestoredNodesBranch::PULL_RESTORED_NODES_QUERY,
                    params,
                )
                .is_err()
                {
                    return;
                }

                res = batch.execute(session).await;
            }
            BranchUpdate::RestoreNode(id) => {
                let mut batch = CharybdisModelBatch::new();
                let params = (vec![id], branch_id);

                if append_statement_or_log_fatal(
                    &mut batch,
                    UpdateRestoredNodesBranch::PUSH_RESTORED_NODES_QUERY,
                    params.clone(),
                )
                .is_err()
                {
                    return;
                }

                if append_statement_or_log_fatal(
                    &mut batch,
                    UpdateDeletedNodesBranch::PULL_DELETED_NODES_QUERY,
                    params.clone(),
                )
                .is_err()
                {
                    return;
                }

                res = batch.execute(session).await;
            }
            BranchUpdate::EditNodeTitle(id) => {
                res = UpdateEditedNodeTitlesBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .push_edited_node_titles(&vec![id])
                .execute(session)
                .await;
            }
            BranchUpdate::EditNodeDescription(id) => {
                res = UpdateEditedNodeDescriptionsBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .push_edited_node_descriptions(&vec![id])
                .execute(session)
                .await;
            }
            BranchUpdate::ReorderNode(reorder_data) => {
                let branch = UpdateReorderedNodes::find_by_id(branch_id).execute(session).await;
                match branch {
                    Ok(mut branch) => {
                        // filter out existing reorder nodes with same id
                        let mut new_reorder_nodes = branch
                            .reordered_nodes
                            .unwrap_or_default()
                            .into_iter()
                            .filter(|node| node.id != reorder_data.id)
                            .collect::<Vec<_>>();

                        new_reorder_nodes.push(reorder_data);
                        branch.reordered_nodes = Some(new_reorder_nodes);

                        res = branch.update().execute(session).await;
                    }
                    Err(err) => {
                        log_fatal(format!("Failed to find branch: {}", err));
                        return;
                    }
                }
            }
            BranchUpdate::CreateWorkflow(id) => {
                res = UpdateCreatedWorkflowsBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .push_created_workflows(&vec![id])
                .execute(session)
                .await;
            }
            BranchUpdate::DeleteWorkflow(id) => {
                res = UpdateDeletedWorkflowsBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .push_deleted_workflows(&vec![id])
                .execute(session)
                .await;
            }
            BranchUpdate::EditWorkflowTitle(id) => {
                res = UpdateEditedWorkflowTitlesBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .push_edited_workflow_titles(&vec![id])
                .execute(session)
                .await;
            }
            BranchUpdate::CreateFlow(id) => {
                res = UpdateCreatedFlowsBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .push_created_flows(&vec![id])
                .execute(session)
                .await;
            }
            BranchUpdate::DeleteFlow(id) => {
                res = UpdateDeletedFlowsBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .push_deleted_flows(&vec![id])
                .execute(session)
                .await;
            }
            BranchUpdate::EditFlowTitle(id) => {
                res = UpdateEditedFlowTitlesBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .push_edited_flow_titles(&vec![id])
                .execute(session)
                .await;
            }
            BranchUpdate::EditFlowDescription(id) => {
                res = UpdateEditedFlowDescriptionsBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .push_edited_flow_descriptions(&vec![id])
                .execute(session)
                .await;
            }
            BranchUpdate::CreateIO(id) => {
                res = UpdateCreatedIOsBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .push_created_ios(&vec![id])
                .execute(session)
                .await;
            }
            BranchUpdate::DeleteIO(id) => {
                res = UpdateDeletedIOsBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .push_deleted_ios(&vec![id])
                .execute(session)
                .await;
            }
            BranchUpdate::EditIOTitle(id) => {
                res = UpdateEditedIOTitlesBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .push_edited_io_titles(&vec![id])
                .execute(session)
                .await;
            }
            BranchUpdate::EditIODescription(id) => {
                res = UpdateEditedIODescriptionsBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .push_edited_io_descriptions(&vec![id])
                .execute(session)
                .await;
            }
            BranchUpdate::CreateFlowStep(id) => {
                res = UpdateCreatedFlowStepsBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .push_created_flow_steps(&vec![id])
                .execute(session)
                .await;
            }
            BranchUpdate::DeleteFlowStep(id) => {
                res = UpdateDeletedFlowStepsBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .push_deleted_flow_steps(&vec![id])
                .execute(session)
                .await;
            }
            BranchUpdate::AppendFlowStepInput(node_id, io_id) => {
                let branch = UpdateCreatedFlowStepInputsByNodeBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .find_by_primary_key()
                .execute(session)
                .await;

                match branch {
                    Ok(mut fs_branch) => {
                        let _ = &mut fs_branch
                            .created_flow_step_inputs_by_node
                            .get_or_insert_with(Default::default)
                            .entry(node_id)
                            .or_insert_with(Set::default)
                            .insert(io_id);

                        res = fs_branch.update().execute(session).await;
                    }
                    Err(err) => {
                        log_fatal(format!("Failed to find branch: {}", err));
                        return;
                    }
                }
            }
            BranchUpdate::RemoveFlowStepInput(node_id, io_id) => {
                let branch = UpdateDeletedFlowStepInputsByNodeBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .find_by_primary_key()
                .execute(session)
                .await;

                match branch {
                    Ok(mut fs_branch) => {
                        let _ = &mut fs_branch
                            .deleted_flow_step_inputs_by_node
                            .get_or_insert_with(Default::default)
                            .entry(node_id)
                            .or_insert_with(Set::default)
                            .insert(io_id);

                        res = fs_branch.update().execute(session).await;
                    }
                    Err(err) => {
                        log_fatal(format!("Failed to find branch: {}", err));
                        return;
                    }
                }
            }
            BranchUpdate::AppendFlowStepOutput(node_id, io_id) => {
                let branch = UpdateCreatedFlowStepOutputsByNodeBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .find_by_primary_key()
                .execute(session)
                .await;

                match branch {
                    Ok(mut fs_branch) => {
                        let _ = &mut fs_branch
                            .created_flow_step_outputs_by_node
                            .get_or_insert_with(Default::default)
                            .entry(node_id)
                            .or_insert_with(Set::default)
                            .insert(io_id);

                        res = fs_branch.update().execute(session).await;
                    }
                    Err(err) => {
                        log_fatal(format!("Failed to find branch: {}", err));
                        return;
                    }
                }
            }
            BranchUpdate::RemoveFlowStepOutput(node_id, io_id) => {
                let branch = UpdateDeletedFlowStepOutputsByNodeBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .find_by_primary_key()
                .execute(session)
                .await;

                match branch {
                    Ok(mut fs_branch) => {
                        let _ = &mut fs_branch
                            .deleted_flow_step_outputs_by_node
                            .get_or_insert_with(Default::default)
                            .entry(node_id)
                            .or_insert_with(Set::default)
                            .insert(io_id);

                        res = fs_branch.update().execute(session).await;
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

        let branch = Branch::find_by_id(branch_id).execute(session).await;

        match branch {
            Ok(mut branch) => {
                let res = branch.check_conflicts(session).await;
                if let Err(err) = res {
                    match err {
                        NodecosmosError::Conflict(_) => (),
                        err => log_fatal(format!("Failed to check_conflicts: {}", err)),
                    }
                }
            }
            Err(err) => {
                log_fatal(format!("Failed to find branch: {}", err));
            }
        }
    }

    pub fn push_title_change_by_object(&mut self, id: Uuid, text_change: TextChange) {
        if let Some(title_change_by_object) = &mut self.title_change_by_object {
            title_change_by_object.insert(id, text_change);
        } else {
            let mut map = Map::new();
            map.insert(id, text_change);
            self.title_change_by_object = Some(map);
        }
    }

    pub fn push_description_change_by_object(&mut self, id: Uuid, text_change: TextChange) {
        if let Some(description_change_by_object) = &mut self.description_change_by_object {
            description_change_by_object.insert(id, text_change);
        } else {
            let mut map = Map::new();
            map.insert(id, text_change);
            self.description_change_by_object = Some(map);
        }
    }
}
