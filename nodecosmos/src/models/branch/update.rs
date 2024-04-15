use charybdis::batch::CharybdisModelBatch;
use charybdis::errors::CharybdisError;
use charybdis::operations::Update;
use charybdis::types::{Frozen, List, Map, Set, Uuid};
use log::error;
use scylla::QueryResult;

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
    UpdateEditedTitleIosBranch, UpdateEditedTitleNodesBranch, UpdateKeptFlowStepsBranch, UpdateReorderedNodes,
    UpdateRestoredFlowStepsBranch, UpdateRestoredFlowsBranch, UpdateRestoredIosBranch, UpdateRestoredNodesBranch,
};
use crate::models::udts::BranchReorderData;

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
    CreatedWorkflowInitialInputs(Map<Uuid, Frozen<List<Uuid>>>),
    DeleteWorkflowInitialInputs(Map<Uuid, Frozen<List<Uuid>>>),
    CreateFlow(Uuid),
    DeleteFlow(Uuid),
    UndoDeleteFlow(Uuid),
    RestoreFlow(Uuid),
    EditFlowTitle(Uuid),
    EditFlowDescription(Uuid),
    CreateFlowStep(Uuid),
    DeleteFlowStep(Uuid),
    UndoDeleteFlowStep(Uuid),
    RestoreFlowStep(Uuid),
    KeepFlowStep(Uuid),
    CreatedFlowStepNodes(Map<Uuid, Frozen<Set<Uuid>>>),
    DeletedFlowStepNodes(Map<Uuid, Frozen<Set<Uuid>>>),
    CreatedFlowStepInputs(Map<Uuid, Frozen<Map<Uuid, Frozen<Set<Uuid>>>>>),
    DeletedFlowStepInputs(Map<Uuid, Frozen<Map<Uuid, Frozen<Set<Uuid>>>>>),
    CreatedFlowStepOutputs(Map<Uuid, Frozen<Map<Uuid, Frozen<Set<Uuid>>>>>),
    DeletedFlowStepOutputs(Map<Uuid, Frozen<Map<Uuid, Frozen<Set<Uuid>>>>>),
    EditFlowStepDescription(Uuid),
    CreateIo(Uuid),
    DeleteIo(Uuid),
    UndoDeleteIo(Uuid),
    RestoreIo(Uuid),
    EditIoTitle(Uuid),
    EditIoDescription(Uuid),
}

impl Branch {
    pub async fn update(data: &RequestData, branch_id: Uuid, update: BranchUpdate) -> Result<Self, NodecosmosError> {
        let res: Result<QueryResult, CharybdisError>;
        let mut check_conflicts = false;

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
                let mut batch: CharybdisModelBatch<&(Vec<Uuid>, Uuid), Branch> = CharybdisModelBatch::new();
                let params = (ids, branch_id);

                res = batch
                    .append_statement(UpdateDeletedNodesBranch::PUSH_DELETED_NODES_QUERY, &params)
                    .append_statement(UpdateCreatedNodesBranch::PULL_CREATED_NODES_QUERY, &params)
                    .append_statement(UpdateRestoredNodesBranch::PULL_RESTORED_NODES_QUERY, &params)
                    .append_statement(
                        UpdateEditedNodeWorkflowsBranch::PULL_EDITED_WORKFLOW_NODES_QUERY,
                        &params,
                    )
                    .execute(data.db_session())
                    .await;

                check_conflicts = true;
            }
            BranchUpdate::UndoDeleteNodes(ids) => {
                res = UpdateDeletedNodesBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .pull_deleted_nodes(ids)
                .execute(data.db_session())
                .await;

                check_conflicts = true;
            }
            BranchUpdate::RestoreNode(id) => {
                let mut batch: CharybdisModelBatch<&(Vec<Uuid>, Uuid), Branch> = CharybdisModelBatch::new();

                let params = (vec![id], branch_id);

                res = batch
                    .append_statement(UpdateRestoredNodesBranch::PUSH_RESTORED_NODES_QUERY, &params)
                    .append_statement(UpdateDeletedNodesBranch::PULL_DELETED_NODES_QUERY, &params)
                    .execute(data.db_session())
                    .await;

                check_conflicts = true;
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

                check_conflicts = true;
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
            BranchUpdate::CreatedWorkflowInitialInputs(created_workflow_initial_inputs) => {
                res = UpdateCreatedWorkflowInitialInputsBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .push_created_workflow_initial_inputs(created_workflow_initial_inputs)
                .execute(data.db_session())
                .await;
            }
            BranchUpdate::DeleteWorkflowInitialInputs(deleted_workflow_initial_inputs) => {
                res = UpdateDeletedWorkflowInitialInputsBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .push_deleted_workflow_initial_inputs(deleted_workflow_initial_inputs)
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
                let mut batch: CharybdisModelBatch<&(Vec<Uuid>, Uuid), Branch> = CharybdisModelBatch::new();

                let params = (vec![id], branch_id);

                res = batch
                    .append_statement(UpdateDeletedFlowsBranch::PUSH_DELETED_FLOWS_QUERY, &params)
                    .append_statement(UpdateCreatedFlowsBranch::PULL_CREATED_FLOWS_QUERY, &params)
                    .append_statement(UpdateRestoredFlowsBranch::PULL_RESTORED_FLOWS_QUERY, &params)
                    .execute(data.db_session())
                    .await;
            }
            BranchUpdate::UndoDeleteFlow(id) => {
                res = UpdateDeletedFlowsBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .pull_deleted_flows(&vec![id])
                .execute(data.db_session())
                .await;

                check_conflicts = true
            }
            BranchUpdate::RestoreFlow(id) => {
                let mut batch: CharybdisModelBatch<&(Vec<Uuid>, Uuid), Branch> = CharybdisModelBatch::new();

                let params = (vec![id], branch_id);

                res = batch
                    .append_statement(UpdateRestoredFlowsBranch::PUSH_RESTORED_FLOWS_QUERY, &params)
                    .append_statement(UpdateDeletedFlowsBranch::PULL_DELETED_FLOWS_QUERY, &params)
                    .execute(data.db_session())
                    .await;

                check_conflicts = true;
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
                let mut batch: CharybdisModelBatch<&(Vec<Uuid>, Uuid), Branch> = CharybdisModelBatch::new();

                let params = (vec![id], branch_id);

                res = batch
                    .append_statement(UpdateDeletedFlowStepsBranch::PUSH_DELETED_FLOW_STEPS_QUERY, &params)
                    .append_statement(UpdateCreatedFlowStepsBranch::PULL_CREATED_FLOW_STEPS_QUERY, &params)
                    .append_statement(UpdateRestoredFlowStepsBranch::PULL_RESTORED_FLOW_STEPS_QUERY, &params)
                    .execute(data.db_session())
                    .await;

                check_conflicts = true;
            }
            BranchUpdate::UndoDeleteFlowStep(id) => {
                res = UpdateDeletedFlowStepsBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .pull_deleted_flow_steps(&vec![id])
                .execute(data.db_session())
                .await;

                check_conflicts = true
            }
            BranchUpdate::RestoreFlowStep(id) => {
                let mut batch: CharybdisModelBatch<&(Vec<Uuid>, Uuid), Branch> = CharybdisModelBatch::new();

                let params = (vec![id], branch_id);

                res = batch
                    .append_statement(UpdateRestoredFlowStepsBranch::PUSH_RESTORED_FLOW_STEPS_QUERY, &params)
                    .append_statement(UpdateDeletedFlowStepsBranch::PULL_DELETED_FLOW_STEPS_QUERY, &params)
                    .execute(data.db_session())
                    .await;

                check_conflicts = true;
            }
            BranchUpdate::KeepFlowStep(id) => {
                res = UpdateKeptFlowStepsBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .push_kept_flow_steps(&vec![id])
                .execute(data.db_session())
                .await;

                check_conflicts = true;
            }
            BranchUpdate::CreatedFlowStepNodes(created_flow_step_nodes) => {
                res = UpdateCreatedFlowStepNodesBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .push_created_flow_step_nodes(created_flow_step_nodes)
                .execute(data.db_session())
                .await;
            }
            BranchUpdate::DeletedFlowStepNodes(deleted_flow_step_nodes) => {
                res = UpdateDeletedFlowStepNodesBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .push_deleted_flow_step_nodes(deleted_flow_step_nodes)
                .execute(data.db_session())
                .await;
            }
            BranchUpdate::CreatedFlowStepInputs(created_flow_step_inputs_by_node) => {
                res = UpdateCreatedFlowStepInputsByNodeBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .push_created_flow_step_inputs_by_node(created_flow_step_inputs_by_node)
                .execute(data.db_session())
                .await;
            }
            BranchUpdate::DeletedFlowStepInputs(deleted_flow_step_inputs_by_node) => {
                res = UpdateDeletedFlowStepInputsByNodeBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .push_deleted_flow_step_inputs_by_node(deleted_flow_step_inputs_by_node)
                .execute(data.db_session())
                .await;
            }
            BranchUpdate::CreatedFlowStepOutputs(created_flow_step_outputs_by_node) => {
                res = UpdateCreatedFlowStepOutputsByNodeBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .push_created_flow_step_outputs_by_node(created_flow_step_outputs_by_node)
                .execute(data.db_session())
                .await;
            }
            BranchUpdate::DeletedFlowStepOutputs(deleted_flow_step_outputs_by_node) => {
                res = UpdateDeletedFlowStepOutputsByNodeBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .push_deleted_flow_step_outputs_by_node(deleted_flow_step_outputs_by_node)
                .execute(data.db_session())
                .await;
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
                let mut batch: CharybdisModelBatch<&(Vec<Uuid>, Uuid), Branch> = CharybdisModelBatch::new();

                let params = (vec![id], branch_id);

                res = batch
                    .append_statement(UpdateDeletedIosBranch::PUSH_DELETED_IOS_QUERY, &params)
                    .append_statement(UpdateCreatedIosBranch::PULL_CREATED_IOS_QUERY, &params)
                    .append_statement(UpdateRestoredIosBranch::PULL_RESTORED_IOS_QUERY, &params)
                    .execute(data.db_session())
                    .await;

                check_conflicts = true;
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
                let mut batch: CharybdisModelBatch<&(Vec<Uuid>, Uuid), Branch> = CharybdisModelBatch::new();

                let params = (vec![id], branch_id);

                res = batch
                    .append_statement(UpdateRestoredIosBranch::PUSH_RESTORED_IOS_QUERY, &params)
                    .append_statement(UpdateDeletedIosBranch::PULL_DELETED_IOS_QUERY, &params)
                    .execute(data.db_session())
                    .await;

                check_conflicts = true;
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
        }

        if let Err(err) = res {
            error!("Failed to update branch: {}", err)
        }

        let mut branch = Branch::find_by_id(branch_id).execute(data.db_session()).await?;

        if check_conflicts {
            println!("Checking conflicts");
            match branch.check_conflicts(data).await {
                Ok(res) => {
                    branch = res;
                }
                Err(err) => {
                    branch = err.branch;
                }
            }
        }

        Ok(branch)
    }
}
