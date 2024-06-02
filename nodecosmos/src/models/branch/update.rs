use charybdis::batch::{CharybdisBatch, CharybdisModelBatch};
use charybdis::errors::CharybdisError;
use charybdis::operations::Update;
use charybdis::types::{Frozen, List, Map, Set, Uuid};
use log::error;
use scylla::{CachingSession, QueryResult};

use crate::errors::NodecosmosError;
use crate::models::branch::{
    Branch, UpdateCreateFlowStepInputsByNodeBranch, UpdateCreateFlowStepNodesBranch,
    UpdateCreateFlowStepOutputsByNodeBranch, UpdateCreateWorkflowInitialInputsBranch, UpdateCreatedFlowStepsBranch,
    UpdateCreatedFlowsBranch, UpdateCreatedIosBranch, UpdateCreatedNodesBranch, UpdateDeleteFlowStepInputsByNodeBranch,
    UpdateDeleteFlowStepNodesBranch, UpdateDeletedFlowStepOutputsByNodeBranch, UpdateDeletedFlowStepsBranch,
    UpdateDeletedFlowsBranch, UpdateDeletedIosBranch, UpdateDeletedNodesBranch,
    UpdateDeletedWorkflowInitialInputsBranch, UpdateEditedDescriptionFlowStepsBranch, UpdateEditedDescriptionIosBranch,
    UpdateEditedDescriptionNodesBranch, UpdateEditedFlowDescriptionBranch, UpdateEditedFlowTitleBranch,
    UpdateEditedNodesBranch, UpdateEditedTitleIosBranch, UpdateEditedTitleNodesBranch, UpdateKeptFlowStepsBranch,
    UpdateReorderedNodes, UpdateRestoredFlowStepsBranch, UpdateRestoredFlowsBranch, UpdateRestoredIosBranch,
    UpdateRestoredNodesBranch,
};
use crate::models::udts::BranchReorderData;

pub enum BranchUpdate {
    CreateNode((Uuid, Set<Uuid>)),
    DeleteNodes(Vec<Uuid>),
    UndoDeleteNodes(Vec<Uuid>),
    RestoreNode(Uuid),
    EditNodeTitle(Uuid),
    EditNodeDescription(Uuid),
    ReorderNode(BranchReorderData),
    EditNode(Uuid),
    CreateWorkflowInitialInputs(Map<Uuid, Frozen<List<Uuid>>>),
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
    CreateFlowStepNodes(Map<Uuid, Frozen<Set<Uuid>>>),
    DeleteFlowStepNodes(Map<Uuid, Frozen<Set<Uuid>>>),
    CreateFlowStepInputs(Map<Uuid, Frozen<Map<Uuid, Frozen<Set<Uuid>>>>>),
    DeleteFlowStepInputs(Map<Uuid, Frozen<Map<Uuid, Frozen<Set<Uuid>>>>>),
    CreateFlowStepOutputs(Map<Uuid, Frozen<Map<Uuid, Frozen<Set<Uuid>>>>>),
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
    pub async fn update(
        db_session: &CachingSession,
        branch_id: Uuid,
        update: BranchUpdate,
    ) -> Result<Self, NodecosmosError> {
        let res: Result<QueryResult, CharybdisError>;
        let mut check_conflicts = false;

        match update {
            BranchUpdate::CreateNode((id, ancestor_ids)) => {
                let created_nodes_branch = UpdateCreatedNodesBranch {
                    id: branch_id,
                    ..Default::default()
                };
                let push_node_q = created_nodes_branch.push_created_nodes(vec![id]);

                let edited_nodes_branch = UpdateEditedNodesBranch {
                    id: branch_id,
                    ..Default::default()
                };
                let push_edited_node_q = edited_nodes_branch.push_edited_nodes(ancestor_ids);

                res = CharybdisBatch::new()
                    .append(push_node_q)
                    .append(push_edited_node_q)
                    .execute(db_session)
                    .await;
            }
            BranchUpdate::DeleteNodes(ids) => {
                let mut batch: CharybdisModelBatch<&(Vec<Uuid>, Uuid), Branch> = CharybdisModelBatch::new();
                let params = (ids, branch_id);

                res = batch
                    .append_statement(UpdateDeletedNodesBranch::PUSH_DELETED_NODES_QUERY, &params)
                    .append_statement(UpdateRestoredNodesBranch::PULL_RESTORED_NODES_QUERY, &params)
                    .append_statement(UpdateEditedNodesBranch::PULL_EDITED_NODES_QUERY, &params)
                    .execute(db_session)
                    .await;

                check_conflicts = true;
            }
            BranchUpdate::UndoDeleteNodes(ids) => {
                res = UpdateDeletedNodesBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .pull_deleted_nodes(ids)
                .execute(db_session)
                .await;

                check_conflicts = true;
            }
            BranchUpdate::RestoreNode(id) => {
                let mut batch: CharybdisModelBatch<&(Vec<Uuid>, Uuid), Branch> = CharybdisModelBatch::new();

                let params = (vec![id], branch_id);

                res = batch
                    .append_statement(UpdateRestoredNodesBranch::PUSH_RESTORED_NODES_QUERY, &params)
                    .append_statement(UpdateDeletedNodesBranch::PULL_DELETED_NODES_QUERY, &params)
                    .execute(db_session)
                    .await;

                check_conflicts = true;
            }
            BranchUpdate::EditNodeTitle(id) => {
                res = UpdateEditedTitleNodesBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .push_edited_title_nodes(&vec![id])
                .execute(db_session)
                .await;
            }
            BranchUpdate::EditNodeDescription(id) => {
                res = UpdateEditedDescriptionNodesBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .push_edited_description_nodes(&vec![id])
                .execute(db_session)
                .await;
            }
            BranchUpdate::ReorderNode(reorder_data) => {
                let branch = UpdateReorderedNodes::find_by_id(branch_id).execute(db_session).await;
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

                        res = branch.update().execute(db_session).await;
                    }
                    Err(err) => {
                        error!("[BranchUpdate::ReorderNode] Failed to find branch: {}", err);
                        return Err(err.into());
                    }
                }

                check_conflicts = true;
            }
            BranchUpdate::EditNode(id) => {
                res = UpdateEditedNodesBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .push_edited_nodes(&vec![id])
                .execute(db_session)
                .await;
            }
            BranchUpdate::CreateWorkflowInitialInputs(created_workflow_initial_inputs) => {
                res = UpdateCreateWorkflowInitialInputsBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .push_created_workflow_initial_inputs(created_workflow_initial_inputs)
                .execute(db_session)
                .await;
            }
            BranchUpdate::DeleteWorkflowInitialInputs(deleted_workflow_initial_inputs) => {
                res = UpdateDeletedWorkflowInitialInputsBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .push_deleted_workflow_initial_inputs(deleted_workflow_initial_inputs)
                .execute(db_session)
                .await;
            }
            BranchUpdate::CreateFlow(id) => {
                res = UpdateCreatedFlowsBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .push_created_flows(&vec![id])
                .execute(db_session)
                .await;
            }
            BranchUpdate::DeleteFlow(id) => {
                let mut batch: CharybdisModelBatch<&(Vec<Uuid>, Uuid), Branch> = CharybdisModelBatch::new();

                let params = (vec![id], branch_id);

                res = batch
                    .append_statement(UpdateDeletedFlowsBranch::PUSH_DELETED_FLOWS_QUERY, &params)
                    .append_statement(UpdateCreatedFlowsBranch::PULL_CREATED_FLOWS_QUERY, &params)
                    .append_statement(UpdateRestoredFlowsBranch::PULL_RESTORED_FLOWS_QUERY, &params)
                    .execute(db_session)
                    .await;
            }
            BranchUpdate::UndoDeleteFlow(id) => {
                res = UpdateDeletedFlowsBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .pull_deleted_flows(&vec![id])
                .execute(db_session)
                .await;

                check_conflicts = true
            }
            BranchUpdate::RestoreFlow(id) => {
                let mut batch: CharybdisModelBatch<&(Vec<Uuid>, Uuid), Branch> = CharybdisModelBatch::new();

                let params = (vec![id], branch_id);

                res = batch
                    .append_statement(UpdateRestoredFlowsBranch::PUSH_RESTORED_FLOWS_QUERY, &params)
                    .append_statement(UpdateDeletedFlowsBranch::PULL_DELETED_FLOWS_QUERY, &params)
                    .execute(db_session)
                    .await;

                check_conflicts = true;
            }
            BranchUpdate::EditFlowTitle(id) => {
                res = UpdateEditedFlowTitleBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .push_edited_title_flows(&vec![id])
                .execute(db_session)
                .await;
            }
            BranchUpdate::EditFlowDescription(id) => {
                res = UpdateEditedFlowDescriptionBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .push_edited_description_flows(&vec![id])
                .execute(db_session)
                .await;
            }
            BranchUpdate::CreateFlowStep(id) => {
                res = UpdateCreatedFlowStepsBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .push_created_flow_steps(&vec![id])
                .execute(db_session)
                .await;
            }
            BranchUpdate::DeleteFlowStep(id) => {
                let mut batch: CharybdisModelBatch<&(Vec<Uuid>, Uuid), Branch> = CharybdisModelBatch::new();

                let params = (vec![id], branch_id);

                res = batch
                    .append_statement(UpdateDeletedFlowStepsBranch::PUSH_DELETED_FLOW_STEPS_QUERY, &params)
                    .append_statement(UpdateCreatedFlowStepsBranch::PULL_CREATED_FLOW_STEPS_QUERY, &params)
                    .append_statement(UpdateRestoredFlowStepsBranch::PULL_RESTORED_FLOW_STEPS_QUERY, &params)
                    .execute(db_session)
                    .await;

                check_conflicts = true;
            }
            BranchUpdate::UndoDeleteFlowStep(id) => {
                res = UpdateDeletedFlowStepsBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .pull_deleted_flow_steps(&vec![id])
                .execute(db_session)
                .await;

                check_conflicts = true
            }
            BranchUpdate::RestoreFlowStep(id) => {
                let mut batch: CharybdisModelBatch<&(Vec<Uuid>, Uuid), Branch> = CharybdisModelBatch::new();

                let params = (vec![id], branch_id);

                res = batch
                    .append_statement(UpdateRestoredFlowStepsBranch::PUSH_RESTORED_FLOW_STEPS_QUERY, &params)
                    .append_statement(UpdateDeletedFlowStepsBranch::PULL_DELETED_FLOW_STEPS_QUERY, &params)
                    .execute(db_session)
                    .await;

                check_conflicts = true;
            }
            BranchUpdate::KeepFlowStep(id) => {
                res = UpdateKeptFlowStepsBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .push_kept_flow_steps(&vec![id])
                .execute(db_session)
                .await;

                check_conflicts = true;
            }
            BranchUpdate::CreateFlowStepNodes(created_flow_step_nodes) => {
                let update_created = UpdateCreateFlowStepNodesBranch {
                    id: branch_id,
                    ..Default::default()
                };

                let update_deleted = UpdateDeleteFlowStepNodesBranch {
                    id: branch_id,
                    ..Default::default()
                };

                let mut batch = CharybdisBatch::new();

                batch
                    .append(update_created.push_created_flow_step_nodes(created_flow_step_nodes.clone()))
                    .append(update_deleted.pull_deleted_flow_step_nodes(created_flow_step_nodes));

                res = batch.execute(db_session).await;
            }
            BranchUpdate::DeleteFlowStepNodes(deleted_flow_step_nodes) => {
                let update_deleted = UpdateDeleteFlowStepNodesBranch {
                    id: branch_id,
                    ..Default::default()
                };

                let update_created = UpdateCreateFlowStepNodesBranch {
                    id: branch_id,
                    ..Default::default()
                };

                let mut batch = CharybdisBatch::new();

                batch
                    .append(update_deleted.push_deleted_flow_step_nodes(deleted_flow_step_nodes.clone()))
                    .append(update_created.pull_created_flow_step_nodes(deleted_flow_step_nodes));

                res = batch.execute(db_session).await;
            }
            BranchUpdate::CreateFlowStepInputs(created_flow_step_inputs_by_node) => {
                res = UpdateCreateFlowStepInputsByNodeBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .push_created_flow_step_inputs_by_node(created_flow_step_inputs_by_node)
                .execute(db_session)
                .await;
            }
            BranchUpdate::DeleteFlowStepInputs(deleted_flow_step_inputs_by_node) => {
                res = UpdateDeleteFlowStepInputsByNodeBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .push_deleted_flow_step_inputs_by_node(deleted_flow_step_inputs_by_node)
                .execute(db_session)
                .await;
            }
            BranchUpdate::CreateFlowStepOutputs(created_flow_step_outputs_by_node) => {
                res = UpdateCreateFlowStepOutputsByNodeBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .push_created_flow_step_outputs_by_node(created_flow_step_outputs_by_node)
                .execute(db_session)
                .await;
            }
            BranchUpdate::DeletedFlowStepOutputs(deleted_flow_step_outputs_by_node) => {
                res = UpdateDeletedFlowStepOutputsByNodeBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .push_deleted_flow_step_outputs_by_node(deleted_flow_step_outputs_by_node)
                .execute(db_session)
                .await;
            }
            BranchUpdate::EditFlowStepDescription(id) => {
                res = UpdateEditedDescriptionFlowStepsBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .push_edited_description_flow_steps(&vec![id])
                .execute(db_session)
                .await;
            }
            BranchUpdate::CreateIo(id) => {
                res = UpdateCreatedIosBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .push_created_ios(&vec![id])
                .execute(db_session)
                .await;
            }
            BranchUpdate::DeleteIo(id) => {
                let mut batch: CharybdisModelBatch<&(Vec<Uuid>, Uuid), Branch> = CharybdisModelBatch::new();

                let params = (vec![id], branch_id);

                res = batch
                    .append_statement(UpdateDeletedIosBranch::PUSH_DELETED_IOS_QUERY, &params)
                    .append_statement(UpdateCreatedIosBranch::PULL_CREATED_IOS_QUERY, &params)
                    .append_statement(UpdateRestoredIosBranch::PULL_RESTORED_IOS_QUERY, &params)
                    .execute(db_session)
                    .await;

                check_conflicts = true;
            }
            BranchUpdate::UndoDeleteIo(id) => {
                res = UpdateDeletedIosBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .pull_deleted_ios(&vec![id])
                .execute(db_session)
                .await;
            }
            BranchUpdate::RestoreIo(id) => {
                let mut batch: CharybdisModelBatch<&(Vec<Uuid>, Uuid), Branch> = CharybdisModelBatch::new();

                let params = (vec![id], branch_id);

                res = batch
                    .append_statement(UpdateRestoredIosBranch::PUSH_RESTORED_IOS_QUERY, &params)
                    .append_statement(UpdateDeletedIosBranch::PULL_DELETED_IOS_QUERY, &params)
                    .execute(db_session)
                    .await;

                check_conflicts = true;
            }
            BranchUpdate::EditIoTitle(id) => {
                res = UpdateEditedTitleIosBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .push_edited_title_ios(&vec![id])
                .execute(db_session)
                .await;
            }
            BranchUpdate::EditIoDescription(id) => {
                res = UpdateEditedDescriptionIosBranch {
                    id: branch_id,
                    ..Default::default()
                }
                .push_edited_description_ios(&vec![id])
                .execute(db_session)
                .await;
            }
        }

        if let Err(err) = res {
            error!("Failed to update branch: {}", err)
        }

        let mut branch = Branch::find_by_id(branch_id).execute(db_session).await?;

        if check_conflicts {
            match branch.check_conflicts(db_session).await {
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
