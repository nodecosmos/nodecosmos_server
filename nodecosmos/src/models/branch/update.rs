use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::branch::{
    Branch, UpdateCreatedFlowStepInputsByNodeBranch, UpdateCreatedFlowStepNodesBranch,
    UpdateCreatedFlowStepOutputsByNodeBranch, UpdateCreatedFlowStepsBranch, UpdateCreatedFlowsBranch,
    UpdateCreatedIosBranch, UpdateCreatedNodesBranch, UpdateCreatedWorkflowInitialInputsBranch,
    UpdateDeletedFlowStepInputsByNodeBranch, UpdateDeletedFlowStepNodesBranch,
    UpdateDeletedFlowStepOutputsByNodeBranch, UpdateDeletedFlowStepsBranch, UpdateDeletedFlowsBranch,
    UpdateDeletedIosBranch, UpdateDeletedNodesBranch, UpdateDeletedWorkflowInitialInputsBranch,
    UpdateEditedDescriptionFlowStepsBranch, UpdateEditedDescriptionIosBranch, UpdateEditedDescriptionNodesBranch,
    UpdateEditedFlowDescriptionBranch, UpdateEditedFlowTitleBranch, UpdateEditedNodeWorkflowsBranch,
    UpdateEditedTitleIosBranch, UpdateEditedTitleNodesBranch, UpdateReorderedNodes, UpdateRestoredFlowStepsBranch,
    UpdateRestoredFlowsBranch, UpdateRestoredIosBranch, UpdateRestoredNodesBranch,
};
use crate::models::traits::Merge;
use crate::models::udts::{BranchReorderData, TextChange};
use crate::models::utils::append_statement_or_log_fatal;
use charybdis::batch::CharybdisModelBatch;
use charybdis::errors::CharybdisError;
use charybdis::operations::Update;
use charybdis::types::{Frozen, List, Map, Set, Uuid};
use log::error;
use scylla::QueryResult;
use std::collections::HashMap;

#[allow(unused)]
pub enum BranchUpdate {
    CreateNode(Uuid),
    DeleteNodes(Vec<Uuid>),
    UndoDeleteNodes(Vec<Uuid>),
    RestoreNode(Uuid),
    EditNodeTitle(Uuid),
    EditNodeDescription(Uuid),
    ReorderNode(BranchReorderData),
    EditNodeWorkflow(Uuid),
    CreatedWorkflowInitialInputs(Uuid, Frozen<List<Uuid>>),
    DeleteWorkflowInitialInputs(Uuid, Frozen<List<Uuid>>),
    CreateFlow(Uuid),
    DeleteFlow(Uuid),
    UndoDeleteFlow(Uuid),
    RestoreFlow(Uuid),
    EditFlowTitle(Uuid),
    EditFlowDescription(Uuid),
    CreateIo(Uuid),
    DeleteIo(Uuid),
    UndoDeleteIo(Uuid),
    RestoreIo(Uuid),
    EditIoTitle(Uuid),
    EditIoDescription(Uuid),
    CreateFlowStep(Uuid),
    DeleteFlowStep(Uuid),
    UndoDeleteFlowStep(Uuid),
    RestoreFlowStep(Uuid),
    CreatedFlowStepNodes(Option<Frozen<Map<Uuid, Frozen<Set<Uuid>>>>>),
    DeletedFlowStepNodes(Option<Frozen<Map<Uuid, Frozen<Set<Uuid>>>>>),
    EditFlowStepDescription(Uuid),
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
            BranchUpdate::DeleteNodes(ids) => {
                let mut batch = CharybdisModelBatch::new();
                let params = (ids, branch_id);

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
            BranchUpdate::UndoDeleteNodes(ids) => {
                res = UpdateDeletedNodesBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .pull_deleted_nodes(ids)
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
                res = UpdateEditedTitleNodesBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .push_edited_title_nodes(&vec![id])
                .execute(data.db_session())
                .await;
            }
            BranchUpdate::EditNodeDescription(id) => {
                res = UpdateEditedDescriptionNodesBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .push_edited_description_nodes(&vec![id])
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
            BranchUpdate::EditNodeWorkflow(id) => {
                res = UpdateEditedNodeWorkflowsBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .push_edited_workflow_nodes(&vec![id])
                .execute(data.db_session())
                .await;
            }
            BranchUpdate::CreatedWorkflowInitialInputs(node_id, created_workflow_initial_inputs) => {
                let mut branch = UpdateCreatedWorkflowInitialInputsBranch::find_by_id(branch_id)
                    .execute(data.db_session())
                    .await?;
                branch
                    .created_workflow_initial_inputs
                    .get_or_insert_with(HashMap::new)
                    .entry(node_id)
                    .or_insert_with(Vec::new)
                    .merge(created_workflow_initial_inputs);

                res = branch.update().execute(data.db_session()).await;
            }
            BranchUpdate::DeleteWorkflowInitialInputs(node_id, deleted_workflow_initial_inputs) => {
                let mut branch = UpdateDeletedWorkflowInitialInputsBranch::find_by_id(branch_id)
                    .execute(data.db_session())
                    .await?;

                branch
                    .deleted_workflow_initial_inputs
                    .get_or_insert_with(HashMap::new)
                    .entry(node_id)
                    .or_insert_with(Vec::new)
                    .merge(deleted_workflow_initial_inputs);

                res = branch.update().execute(data.db_session()).await;
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
                let mut batch = CharybdisModelBatch::new();
                let params = (vec![id], branch_id);

                append_statement_or_log_fatal(
                    &mut batch,
                    UpdateDeletedFlowsBranch::PUSH_DELETED_FLOWS_QUERY,
                    params.clone(),
                )?;

                append_statement_or_log_fatal(
                    &mut batch,
                    UpdateCreatedFlowsBranch::PULL_CREATED_FLOWS_QUERY,
                    params.clone(),
                )?;

                append_statement_or_log_fatal(
                    &mut batch,
                    UpdateRestoredFlowsBranch::PULL_RESTORED_FLOWS_QUERY,
                    params,
                )?;

                res = batch.execute(data.db_session()).await;
            }
            BranchUpdate::UndoDeleteFlow(id) => {
                res = UpdateDeletedFlowsBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .pull_deleted_flows(&vec![id])
                .execute(data.db_session())
                .await;
            }
            BranchUpdate::RestoreFlow(id) => {
                let mut batch = CharybdisModelBatch::new();
                let params = (vec![id], branch_id);

                append_statement_or_log_fatal(
                    &mut batch,
                    UpdateRestoredFlowsBranch::PUSH_RESTORED_FLOWS_QUERY,
                    params.clone(),
                )?;

                append_statement_or_log_fatal(
                    &mut batch,
                    UpdateDeletedFlowsBranch::PULL_DELETED_FLOWS_QUERY,
                    params.clone(),
                )?;

                res = batch.execute(data.db_session()).await;
            }
            BranchUpdate::EditFlowTitle(id) => {
                res = UpdateEditedFlowTitleBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .push_edited_title_flows(&vec![id])
                .execute(data.db_session())
                .await;
            }
            BranchUpdate::EditFlowDescription(id) => {
                res = UpdateEditedFlowDescriptionBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .push_edited_description_flows(&vec![id])
                .execute(data.db_session())
                .await;
            }
            BranchUpdate::CreateIo(id) => {
                res = UpdateCreatedIosBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .push_created_ios(&vec![id])
                .execute(data.db_session())
                .await;
            }
            BranchUpdate::DeleteIo(id) => {
                let mut batch = CharybdisModelBatch::new();
                let params = (vec![id], branch_id);

                append_statement_or_log_fatal(
                    &mut batch,
                    UpdateDeletedIosBranch::PUSH_DELETED_IOS_QUERY,
                    params.clone(),
                )?;

                append_statement_or_log_fatal(
                    &mut batch,
                    UpdateCreatedIosBranch::PULL_CREATED_IOS_QUERY,
                    params.clone(),
                )?;

                append_statement_or_log_fatal(&mut batch, UpdateRestoredIosBranch::PULL_RESTORED_IOS_QUERY, params)?;

                res = batch.execute(data.db_session()).await;
            }
            BranchUpdate::UndoDeleteIo(id) => {
                res = UpdateDeletedIosBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .pull_deleted_ios(&vec![id])
                .execute(data.db_session())
                .await;
            }
            BranchUpdate::RestoreIo(id) => {
                let mut batch = CharybdisModelBatch::new();
                let params = (vec![id], branch_id);

                append_statement_or_log_fatal(
                    &mut batch,
                    UpdateRestoredIosBranch::PUSH_RESTORED_IOS_QUERY,
                    params.clone(),
                )?;

                append_statement_or_log_fatal(
                    &mut batch,
                    UpdateDeletedIosBranch::PULL_DELETED_IOS_QUERY,
                    params.clone(),
                )?;

                res = batch.execute(data.db_session()).await;
            }
            BranchUpdate::EditIoTitle(id) => {
                res = UpdateEditedTitleIosBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .push_edited_title_ios(&vec![id])
                .execute(data.db_session())
                .await;
            }
            BranchUpdate::EditIoDescription(id) => {
                res = UpdateEditedDescriptionIosBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .push_edited_description_ios(&vec![id])
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
                let mut batch = CharybdisModelBatch::new();
                let params = (vec![id], branch_id);

                append_statement_or_log_fatal(
                    &mut batch,
                    UpdateDeletedFlowStepsBranch::PUSH_DELETED_FLOW_STEPS_QUERY,
                    params.clone(),
                )?;

                append_statement_or_log_fatal(
                    &mut batch,
                    UpdateCreatedFlowStepsBranch::PULL_CREATED_FLOW_STEPS_QUERY,
                    params.clone(),
                )?;

                append_statement_or_log_fatal(
                    &mut batch,
                    UpdateRestoredFlowStepsBranch::PULL_RESTORED_FLOW_STEPS_QUERY,
                    params,
                )?;

                res = batch.execute(data.db_session()).await;
            }
            BranchUpdate::UndoDeleteFlowStep(id) => {
                res = UpdateDeletedFlowStepsBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .pull_deleted_flow_steps(&vec![id])
                .execute(data.db_session())
                .await;
            }
            BranchUpdate::RestoreFlowStep(id) => {
                let mut batch = CharybdisModelBatch::new();
                let params = (vec![id], branch_id);

                append_statement_or_log_fatal(
                    &mut batch,
                    UpdateRestoredFlowStepsBranch::PUSH_RESTORED_FLOW_STEPS_QUERY,
                    params.clone(),
                )?;

                append_statement_or_log_fatal(
                    &mut batch,
                    UpdateDeletedFlowStepsBranch::PULL_DELETED_FLOW_STEPS_QUERY,
                    params.clone(),
                )?;

                res = batch.execute(data.db_session()).await;
            }
            BranchUpdate::EditFlowStepDescription(id) => {
                res = UpdateEditedDescriptionFlowStepsBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .push_edited_description_flow_steps(&vec![id])
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
