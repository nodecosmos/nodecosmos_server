use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::branch::{
    Branch, UpdateCreatedFlowStepInputsByNodeBranch, UpdateCreatedFlowStepNodesBranch,
    UpdateCreatedFlowStepOutputsByNodeBranch, UpdateCreatedFlowStepsBranch, UpdateCreatedFlowsBranch,
    UpdateCreatedIOsBranch, UpdateCreatedNodesBranch, UpdateCreatedWorkflowInitialInputsBranch,
    UpdateCreatedWorkflowsBranch, UpdateDeletedFlowStepInputsByNodeBranch, UpdateDeletedFlowStepNodesBranch,
    UpdateDeletedFlowStepOutputsByNodeBranch, UpdateDeletedFlowStepsBranch, UpdateDeletedFlowsBranch,
    UpdateDeletedIOsBranch, UpdateDeletedNodesBranch, UpdateDeletedWorkflowInitialInputsBranch,
    UpdateDeletedWorkflowsBranch, UpdateEditedFlowDescriptionsBranch, UpdateEditedFlowTitlesBranch,
    UpdateEditedIODescriptionsBranch, UpdateEditedIOTitlesBranch, UpdateEditedNodeDescriptionsBranch,
    UpdateEditedNodeTitlesBranch, UpdateEditedWorkflowTitlesBranch, UpdateReorderedNodes, UpdateRestoredNodesBranch,
};
use crate::models::udts::{BranchReorderData, TextChange};
use crate::models::utils::append_statement_or_log_fatal;
use charybdis::batch::CharybdisModelBatch;
use charybdis::errors::CharybdisError;
use charybdis::operations::Update;
use charybdis::types::{Frozen, Map, Set, Uuid};
use log::error;
use scylla::QueryResult;

#[allow(unused)]
pub enum BranchUpdate {
    CreateNode(Uuid),
    DeleteNode(Uuid),
    UndoDeleteNode(Uuid),
    RestoreNode(Uuid),
    EditNodeTitle(Uuid),
    EditNodeDescription(Uuid),
    ReorderNode(BranchReorderData),
    CreateWorkflow(Uuid),
    DeleteWorkflow(Uuid),
    CreatedWorkflowInitialInputs(Vec<Uuid>),
    DeleteWorkflowInitialInputs(Vec<Uuid>),
    EditWorkflowTitle(Uuid),
    CreateFlow(Uuid),
    DeleteFlow(Uuid),
    EditFlowTitle(Uuid),
    EditFlowDescription(Uuid),
    CreateIo(Uuid),
    DeleteIo(Uuid),
    EditIOTitle(Uuid),
    EditIODescription(Uuid),
    CreateFlowStep(Uuid),
    DeleteFlowStep(Uuid),
    CreatedFlowStepNodes(Option<Frozen<Map<Uuid, Frozen<Set<Uuid>>>>>),
    DeletedFlowStepNodes(Option<Frozen<Map<Uuid, Frozen<Set<Uuid>>>>>),
    CreatedFlowStepInputs(Option<Frozen<Map<Uuid, Frozen<Map<Uuid, Frozen<Set<Uuid>>>>>>>),
    DeletedFlowStepInputs(Option<Frozen<Map<Uuid, Frozen<Map<Uuid, Frozen<Set<Uuid>>>>>>>),
    CreatedFlowStepOutputs(Option<Frozen<Map<Uuid, Frozen<Map<Uuid, Frozen<Set<Uuid>>>>>>>),
    DeletedFlowStepOutputs(Option<Frozen<Map<Uuid, Frozen<Map<Uuid, Frozen<Set<Uuid>>>>>>>),
}

impl Branch {
    async fn check_branch_conflicts(data: &RequestData, branch_id: Uuid) {
        let data = data.clone();

        tokio::spawn(async move {
            let branch = Branch::find_by_id(branch_id).execute(data.db_session()).await;

            return match branch {
                Ok(mut branch) => {
                    let res = branch.check_conflicts(data.db_session()).await;

                    if let Err(err) = res {
                        error!("Failed to check_conflicts: {}", err);
                    }
                }
                Err(err) => error!("Failed to find branch: {}", err),
            };
        });
    }

    pub async fn update(data: &RequestData, branch_id: Uuid, update: BranchUpdate) -> Result<(), NodecosmosError> {
        let res: Result<QueryResult, CharybdisError>;

        match update {
            BranchUpdate::CreateNode(id) => {
                res = UpdateCreatedNodesBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .push_created_nodes(&vec![id])
                .execute(data.db_session())
                .await;
            }
            BranchUpdate::DeleteNode(id) => {
                let mut batch = CharybdisModelBatch::new();
                let params = (vec![id], branch_id);

                append_statement_or_log_fatal(
                    &mut batch,
                    UpdateDeletedNodesBranch::PUSH_DELETED_NODES_QUERY,
                    params.clone(),
                )?;

                append_statement_or_log_fatal(
                    &mut batch,
                    UpdateCreatedNodesBranch::PULL_CREATED_NODES_QUERY,
                    params.clone(),
                )?;

                append_statement_or_log_fatal(
                    &mut batch,
                    UpdateRestoredNodesBranch::PULL_RESTORED_NODES_QUERY,
                    params,
                )?;

                res = batch.execute(data.db_session()).await;
            }
            BranchUpdate::UndoDeleteNode(id) => {
                res = UpdateDeletedNodesBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .pull_deleted_nodes(&vec![id])
                .execute(data.db_session())
                .await;
            }
            BranchUpdate::RestoreNode(id) => {
                let mut batch = CharybdisModelBatch::new();
                let params = (vec![id], branch_id);

                append_statement_or_log_fatal(
                    &mut batch,
                    UpdateRestoredNodesBranch::PUSH_RESTORED_NODES_QUERY,
                    params.clone(),
                )?;

                append_statement_or_log_fatal(
                    &mut batch,
                    UpdateDeletedNodesBranch::PULL_DELETED_NODES_QUERY,
                    params.clone(),
                )?;

                res = batch.execute(data.db_session()).await;
            }
            BranchUpdate::EditNodeTitle(id) => {
                res = UpdateEditedNodeTitlesBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .push_edited_node_titles(&vec![id])
                .execute(data.db_session())
                .await;
            }
            BranchUpdate::EditNodeDescription(id) => {
                res = UpdateEditedNodeDescriptionsBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .push_edited_node_descriptions(&vec![id])
                .execute(data.db_session())
                .await;
            }
            BranchUpdate::ReorderNode(reorder_data) => {
                let branch = UpdateReorderedNodes::find_by_id(branch_id)
                    .execute(data.db_session())
                    .await;
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

                        res = branch.update().execute(data.db_session()).await;
                    }
                    Err(err) => {
                        error!("[BranchUpdate::ReorderNode] Failed to find branch: {}", err);
                        return Err(err.into());
                    }
                }
            }
            BranchUpdate::CreateWorkflow(id) => {
                res = UpdateCreatedWorkflowsBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .push_created_workflows(&vec![id])
                .execute(data.db_session())
                .await;
            }
            BranchUpdate::DeleteWorkflow(id) => {
                res = UpdateDeletedWorkflowsBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .push_deleted_workflows(&vec![id])
                .execute(data.db_session())
                .await;
            }
            BranchUpdate::CreatedWorkflowInitialInputs(ids) => {
                res = UpdateCreatedWorkflowInitialInputsBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .push_created_workflow_initial_inputs(&ids)
                .execute(data.db_session())
                .await;
            }
            BranchUpdate::DeleteWorkflowInitialInputs(ids) => {
                res = UpdateDeletedWorkflowInitialInputsBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .push_deleted_workflow_initial_inputs(&ids)
                .execute(data.db_session())
                .await;
            }
            BranchUpdate::EditWorkflowTitle(id) => {
                res = UpdateEditedWorkflowTitlesBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .push_edited_workflow_titles(&vec![id])
                .execute(data.db_session())
                .await;
            }
            BranchUpdate::CreateFlow(id) => {
                res = UpdateCreatedFlowsBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .push_created_flows(&vec![id])
                .execute(data.db_session())
                .await;
            }
            BranchUpdate::DeleteFlow(id) => {
                res = UpdateDeletedFlowsBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .push_deleted_flows(&vec![id])
                .execute(data.db_session())
                .await;
            }
            BranchUpdate::EditFlowTitle(id) => {
                res = UpdateEditedFlowTitlesBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .push_edited_flow_titles(&vec![id])
                .execute(data.db_session())
                .await;
            }
            BranchUpdate::EditFlowDescription(id) => {
                res = UpdateEditedFlowDescriptionsBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .push_edited_flow_descriptions(&vec![id])
                .execute(data.db_session())
                .await;
            }
            BranchUpdate::CreateIo(id) => {
                res = UpdateCreatedIOsBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .push_created_ios(&vec![id])
                .execute(data.db_session())
                .await;
            }
            BranchUpdate::DeleteIo(id) => {
                res = UpdateDeletedIOsBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .push_deleted_ios(&vec![id])
                .execute(data.db_session())
                .await;
            }
            BranchUpdate::EditIOTitle(id) => {
                res = UpdateEditedIOTitlesBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .push_edited_io_titles(&vec![id])
                .execute(data.db_session())
                .await;
            }
            BranchUpdate::EditIODescription(id) => {
                res = UpdateEditedIODescriptionsBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .push_edited_io_descriptions(&vec![id])
                .execute(data.db_session())
                .await;
            }
            BranchUpdate::CreateFlowStep(id) => {
                res = UpdateCreatedFlowStepsBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .push_created_flow_steps(&vec![id])
                .execute(data.db_session())
                .await;
            }
            BranchUpdate::DeleteFlowStep(id) => {
                res = UpdateDeletedFlowStepsBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .push_deleted_flow_steps(&vec![id])
                .execute(data.db_session())
                .await;
            }
            BranchUpdate::CreatedFlowStepNodes(created_flow_step_nodes) => {
                res = UpdateCreatedFlowStepNodesBranch {
                    id: branch_id,
                    created_flow_step_nodes,
                }
                .update()
                .execute(data.db_session())
                .await;
            }
            BranchUpdate::DeletedFlowStepNodes(deleted_flow_step_nodes) => {
                res = UpdateDeletedFlowStepNodesBranch {
                    id: branch_id,
                    deleted_flow_step_nodes,
                }
                .update()
                .execute(data.db_session())
                .await;
            }
            BranchUpdate::CreatedFlowStepInputs(created_flow_step_inputs_by_node) => {
                res = UpdateCreatedFlowStepInputsByNodeBranch {
                    id: branch_id,
                    created_flow_step_inputs_by_node,
                }
                .update()
                .execute(data.db_session())
                .await;
            }
            BranchUpdate::DeletedFlowStepInputs(deleted_flow_step_inputs_by_node) => {
                res = UpdateDeletedFlowStepInputsByNodeBranch {
                    id: branch_id,
                    deleted_flow_step_inputs_by_node,
                }
                .update()
                .execute(data.db_session())
                .await;
            }
            BranchUpdate::CreatedFlowStepOutputs(created_flow_step_outputs_by_node) => {
                res = UpdateCreatedFlowStepOutputsByNodeBranch {
                    id: branch_id,
                    created_flow_step_outputs_by_node,
                }
                .update()
                .execute(data.db_session())
                .await;
            }
            BranchUpdate::DeletedFlowStepOutputs(deleted_flow_step_outputs_by_node) => {
                res = UpdateDeletedFlowStepOutputsByNodeBranch {
                    id: branch_id,
                    deleted_flow_step_outputs_by_node,
                }
                .update()
                .execute(data.db_session())
                .await;
            }
        }

        if let Err(err) = res {
            error!("Failed to update branch: {}", err)
        }

        Self::check_branch_conflicts(data, branch_id).await;

        Ok(())
    }

    #[allow(unused)]
    pub async fn undo_update(data: &RequestData, branch_id: Uuid, update: BranchUpdate) -> Result<(), NodecosmosError> {
        let res: Result<QueryResult, CharybdisError>;

        match update {
            BranchUpdate::CreateNode(id) => {
                res = UpdateCreatedNodesBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .pull_created_nodes(&vec![id])
                .execute(data.db_session())
                .await;
            }
            BranchUpdate::DeleteNode(id) => {
                res = UpdateDeletedNodesBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .pull_deleted_nodes(&vec![id])
                .execute(data.db_session())
                .await;
            }
            BranchUpdate::UndoDeleteNode(id) => {
                res = UpdateDeletedNodesBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .push_deleted_nodes(&vec![id])
                .execute(data.db_session())
                .await;
            }
            BranchUpdate::RestoreNode(id) => {
                let mut batch = CharybdisModelBatch::new();
                let params = (vec![id], branch_id);

                append_statement_or_log_fatal(
                    &mut batch,
                    UpdateRestoredNodesBranch::PUSH_RESTORED_NODES_QUERY,
                    params.clone(),
                )?;

                append_statement_or_log_fatal(
                    &mut batch,
                    UpdateDeletedNodesBranch::PULL_DELETED_NODES_QUERY,
                    params.clone(),
                )?;

                res = batch.execute(data.db_session()).await;
            }
            BranchUpdate::EditNodeTitle(id) => {
                res = UpdateEditedNodeTitlesBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .pull_edited_node_titles(&vec![id])
                .execute(data.db_session())
                .await;
            }
            BranchUpdate::EditNodeDescription(id) => {
                res = UpdateEditedNodeDescriptionsBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .pull_edited_node_descriptions(&vec![id])
                .execute(data.db_session())
                .await;
            }
            BranchUpdate::ReorderNode(reorder_data) => {
                let branch = UpdateReorderedNodes::find_by_id(branch_id)
                    .execute(data.db_session())
                    .await;
                match branch {
                    Ok(mut branch) => {
                        // filter out existing reorder nodes with same id
                        let new_reorder_nodes = branch
                            .reordered_nodes
                            .unwrap_or_default()
                            .into_iter()
                            .filter(|node| node.id != reorder_data.id)
                            .collect::<Vec<_>>();

                        branch.reordered_nodes = Some(new_reorder_nodes);

                        res = branch.update().execute(data.db_session()).await;
                    }
                    Err(err) => {
                        error!("[BranchUpdate::ReorderNode] Failed to find branch: {}", err);
                        return Err(err.into());
                    }
                }
            }
            BranchUpdate::CreateWorkflow(id) => {
                res = UpdateCreatedWorkflowsBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .pull_created_workflows(&vec![id])
                .execute(data.db_session())
                .await;
            }
            BranchUpdate::DeleteWorkflow(id) => {
                res = UpdateDeletedWorkflowsBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .pull_deleted_workflows(&vec![id])
                .execute(data.db_session())
                .await;
            }
            BranchUpdate::CreatedWorkflowInitialInputs(ids) => {
                res = UpdateCreatedWorkflowInitialInputsBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .pull_created_workflow_initial_inputs(&ids)
                .execute(data.db_session())
                .await;
            }
            BranchUpdate::DeleteWorkflowInitialInputs(ids) => {
                res = UpdateDeletedWorkflowInitialInputsBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .pull_deleted_workflow_initial_inputs(&ids)
                .execute(data.db_session())
                .await;
            }
            BranchUpdate::EditWorkflowTitle(id) => {
                res = UpdateEditedWorkflowTitlesBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .pull_edited_workflow_titles(&vec![id])
                .execute(data.db_session())
                .await;
            }
            BranchUpdate::CreateFlow(id) => {
                res = UpdateCreatedFlowsBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .pull_created_flows(&vec![id])
                .execute(data.db_session())
                .await;
            }
            BranchUpdate::DeleteFlow(id) => {
                res = UpdateDeletedFlowsBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .pull_deleted_flows(&vec![id])
                .execute(data.db_session())
                .await;
            }
            BranchUpdate::EditFlowTitle(id) => {
                res = UpdateEditedFlowTitlesBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .pull_edited_flow_titles(&vec![id])
                .execute(data.db_session())
                .await;
            }
            BranchUpdate::EditFlowDescription(id) => {
                res = UpdateEditedFlowDescriptionsBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .pull_edited_flow_descriptions(&vec![id])
                .execute(data.db_session())
                .await;
            }
            BranchUpdate::CreateIo(id) => {
                res = UpdateCreatedIOsBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .pull_created_ios(&vec![id])
                .execute(data.db_session())
                .await;
            }
            BranchUpdate::DeleteIo(id) => {
                res = UpdateDeletedIOsBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .pull_deleted_ios(&vec![id])
                .execute(data.db_session())
                .await;
            }
            BranchUpdate::EditIOTitle(id) => {
                res = UpdateEditedIOTitlesBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .pull_edited_io_titles(&vec![id])
                .execute(data.db_session())
                .await;
            }
            BranchUpdate::EditIODescription(id) => {
                res = UpdateEditedIODescriptionsBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .pull_edited_io_descriptions(&vec![id])
                .execute(data.db_session())
                .await;
            }
            BranchUpdate::CreateFlowStep(id) => {
                res = UpdateCreatedFlowStepsBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .pull_created_flow_steps(&vec![id])
                .execute(data.db_session())
                .await;
            }
            BranchUpdate::DeleteFlowStep(id) => {
                res = UpdateDeletedFlowStepsBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .pull_deleted_flow_steps(&vec![id])
                .execute(data.db_session())
                .await;
            }
            BranchUpdate::CreatedFlowStepNodes(created_flow_step_nodes) => {
                res = UpdateCreatedFlowStepNodesBranch {
                    id: branch_id,
                    created_flow_step_nodes,
                }
                .update()
                .execute(data.db_session())
                .await;
            }
            BranchUpdate::DeletedFlowStepNodes(deleted_flow_step_nodes) => {
                res = UpdateDeletedFlowStepNodesBranch {
                    id: branch_id,
                    deleted_flow_step_nodes,
                }
                .update()
                .execute(data.db_session())
                .await;
            }
            BranchUpdate::CreatedFlowStepInputs(created_flow_step_inputs_by_node) => {
                res = UpdateCreatedFlowStepInputsByNodeBranch {
                    id: branch_id,
                    created_flow_step_inputs_by_node,
                }
                .update()
                .execute(data.db_session())
                .await;
            }
            BranchUpdate::DeletedFlowStepInputs(deleted_flow_step_inputs_by_node) => {
                res = UpdateDeletedFlowStepInputsByNodeBranch {
                    id: branch_id,
                    deleted_flow_step_inputs_by_node,
                }
                .update()
                .execute(data.db_session())
                .await;
            }
            BranchUpdate::CreatedFlowStepOutputs(created_flow_step_outputs_by_node) => {
                res = UpdateCreatedFlowStepOutputsByNodeBranch {
                    id: branch_id,
                    created_flow_step_outputs_by_node,
                }
                .update()
                .execute(data.db_session())
                .await;
            }
            BranchUpdate::DeletedFlowStepOutputs(deleted_flow_step_outputs_by_node) => {
                res = UpdateDeletedFlowStepOutputsByNodeBranch {
                    id: branch_id,
                    deleted_flow_step_outputs_by_node,
                }
                .update()
                .execute(data.db_session())
                .await;
            }
        }

        if let Err(err) = res {
            error!("Failed to undo update branch: {}", err)
        }

        Self::check_branch_conflicts(data, branch_id).await;

        Ok(())
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
