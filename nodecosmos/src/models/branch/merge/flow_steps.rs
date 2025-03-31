use std::collections::HashMap;

use anyhow::Context;
use charybdis::operations::{Delete, DeleteWithCallbacks, Insert, InsertWithCallbacks, UpdateWithCallbacks};
use charybdis::types::{Set, Uuid};
use scylla::client::caching_session::CachingSession;
use serde::{Deserialize, Serialize};

use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::branch::Branch;
use crate::models::flow_step::{FlowStep, PkFlowStep, UpdateInputIdsFlowStep, UpdateNodeIdsFlowStep};
use crate::models::traits::{Branchable, FindForBranchMerge, FlowId, Id, IncrementFraction, NodeId, Reload};
use crate::models::traits::{ModelContext, PluckFromStream};

#[derive(Serialize, Deserialize)]
pub struct MergeFlowSteps {
    pub restored_flow_steps: Option<Vec<FlowStep>>,
    pub created_flow_steps: Option<Vec<FlowStep>>,
    pub deleted_flow_steps: Option<Vec<FlowStep>>,
    pub created_fs_nodes_flow_steps: Option<Vec<UpdateNodeIdsFlowStep>>,
    pub branched_created_fs_nodes_flow_steps: Option<HashMap<Uuid, UpdateNodeIdsFlowStep>>,
    pub deleted_fs_nodes_flow_steps: Option<Vec<UpdateNodeIdsFlowStep>>,
    pub created_fs_inputs_flow_steps: Option<Vec<UpdateInputIdsFlowStep>>,
    pub branched_created_fs_inputs_flow_steps: Option<HashMap<Uuid, UpdateNodeIdsFlowStep>>,
    pub deleted_fs_inputs_flow_steps: Option<Vec<UpdateInputIdsFlowStep>>,
    // Delta fields that are calculated during merge
    pub added_node_ids_by_flow_step: Option<HashMap<Uuid, Vec<Uuid>>>,
    pub removed_node_ids_by_flow_step: Option<HashMap<Uuid, Vec<Uuid>>>,
    pub added_input_ids_by_flow_step: Option<HashMap<Uuid, HashMap<Uuid, Vec<Uuid>>>>,
    pub removed_input_ids_by_flow_step: Option<HashMap<Uuid, HashMap<Uuid, Vec<Uuid>>>>,
    pub added_output_ids_by_flow_step: Option<HashMap<Uuid, HashMap<Uuid, Vec<Uuid>>>>,
    pub removed_output_ids_by_flow_step: Option<HashMap<Uuid, HashMap<Uuid, Vec<Uuid>>>>,
}

impl MergeFlowSteps {
    pub async fn restored_flow_steps(
        db_session: &CachingSession,
        branch: &Branch,
    ) -> Result<Option<Vec<FlowStep>>, NodecosmosError> {
        if let Some(restored_flow_step_ids) = &branch.restored_flow_steps {
            let already_restored_ids =
                PkFlowStep::find_by_branch_id_and_ids(db_session, branch.original_id(), restored_flow_step_ids)
                    .await
                    .pluck_id_set()
                    .await?;
            let fs_stream = FlowStep::find_by_branch_id_and_ids(db_session, branch.id, restored_flow_step_ids).await;
            let mut flow_steps = branch.filter_out_flow_steps_with_deleted_parents(fs_stream).await?;

            flow_steps.retain(|flow_step| !already_restored_ids.contains(&flow_step.id));

            return Ok(Some(flow_steps));
        }

        Ok(None)
    }

    pub async fn created_flow_steps(
        db_session: &CachingSession,
        branch: &Branch,
    ) -> Result<Option<Vec<FlowStep>>, NodecosmosError> {
        if let Some(created_flow_step_ids) = &branch.created_flow_steps {
            let fs_stream = FlowStep::find_by_branch_id_and_ids(db_session, branch.id, created_flow_step_ids).await;
            let flow_steps = branch.filter_out_flow_steps_with_deleted_parents(fs_stream).await?;

            return Ok(Some(flow_steps));
        }

        Ok(None)
    }

    pub async fn deleted_flow_steps(
        db_session: &CachingSession,
        branch: &Branch,
    ) -> Result<Option<Vec<FlowStep>>, NodecosmosError> {
        if let Some(deleted_flow_step_ids) = &branch.deleted_flow_steps {
            let fs_stream =
                FlowStep::find_by_branch_id_and_ids(db_session, branch.original_id(), deleted_flow_step_ids).await;
            let flow_steps = branch.filter_out_flow_steps_with_deleted_parents(fs_stream).await?;

            return Ok(Some(flow_steps));
        }

        Ok(None)
    }

    fn filter_out_deleted_flow_steps<FsType>(branch: &Branch, flow_steps: Vec<FsType>) -> Vec<FsType>
    where
        FsType: FlowId + NodeId + Id,
    {
        flow_steps
            .into_iter()
            .filter(|flow_step| {
                if branch
                    .deleted_nodes
                    .as_ref()
                    .is_some_and(|dfs| dfs.contains(&flow_step.node_id()))
                {
                    return false;
                }

                if branch
                    .deleted_flows
                    .as_ref()
                    .is_some_and(|dfs| dfs.contains(&flow_step.flow_id()))
                {
                    return false;
                }

                if branch
                    .deleted_flow_steps
                    .as_ref()
                    .is_some_and(|dfs| dfs.contains(&flow_step.id()))
                {
                    return false;
                }

                true
            })
            .collect()
    }

    // Returns original flow steps
    pub async fn created_fs_nodes_flow_steps(
        db_session: &CachingSession,
        branch: &Branch,
    ) -> Result<Option<Vec<UpdateNodeIdsFlowStep>>, NodecosmosError> {
        if let Some(created_flow_step_nodes) = &branch.created_flow_step_nodes {
            let flow_step_ids: Set<Uuid> = created_flow_step_nodes.keys().cloned().collect();
            let flow_steps =
                UpdateNodeIdsFlowStep::find_by_branch_id_and_ids(db_session, branch.original_id(), &flow_step_ids)
                    .await
                    .try_collect()
                    .await?;

            return Ok(Some(Self::filter_out_deleted_flow_steps(branch, flow_steps)));
        }

        Ok(None)
    }

    pub async fn branched_created_fs_nodes_flow_steps(
        db_session: &CachingSession,
        branch: &Branch,
    ) -> Result<Option<HashMap<Uuid, UpdateNodeIdsFlowStep>>, NodecosmosError> {
        if let Some(created_flow_step_nodes) = &branch.created_flow_step_nodes {
            let flow_step_ids: Set<Uuid> = created_flow_step_nodes.keys().cloned().collect();
            let flow_steps = UpdateNodeIdsFlowStep::find_by_branch_id_and_ids(db_session, branch.id, &flow_step_ids)
                .await
                .try_collect()
                .await?;

            return Ok(Some(flow_steps.into_iter().map(|fs| (fs.id, fs)).collect()));
        }

        Ok(None)
    }

    // Returns original flow steps
    pub async fn deleted_fs_nodes_flow_steps(
        db_session: &CachingSession,
        branch: &Branch,
    ) -> Result<Option<Vec<UpdateNodeIdsFlowStep>>, NodecosmosError> {
        if let Some(deleted_flow_step_nodes) = &branch.deleted_flow_step_nodes {
            let flow_step_ids: Set<Uuid> = deleted_flow_step_nodes.keys().cloned().collect();
            let flow_steps =
                UpdateNodeIdsFlowStep::find_by_branch_id_and_ids(db_session, branch.original_id(), &flow_step_ids)
                    .await
                    .try_collect()
                    .await?;

            return Ok(Some(Self::filter_out_deleted_flow_steps(branch, flow_steps)));
        }

        Ok(None)
    }

    // Returns original flow steps
    pub async fn created_fs_inputs_flow_steps(
        db_session: &CachingSession,
        branch: &Branch,
    ) -> Result<Option<Vec<UpdateInputIdsFlowStep>>, NodecosmosError> {
        if let Some(created_flow_step_inputs_by_node) = &branch.created_flow_step_inputs_by_node {
            let flow_step_ids: Set<Uuid> = created_flow_step_inputs_by_node.keys().cloned().collect();
            let flow_steps =
                UpdateInputIdsFlowStep::find_by_branch_id_and_ids(db_session, branch.original_id(), &flow_step_ids)
                    .await
                    .try_collect()
                    .await?;

            return Ok(Some(Self::filter_out_deleted_flow_steps(branch, flow_steps)));
        }

        Ok(None)
    }

    pub async fn branched_created_fs_inputs_flow_steps(
        db_session: &CachingSession,
        branch: &Branch,
    ) -> Result<Option<HashMap<Uuid, UpdateNodeIdsFlowStep>>, NodecosmosError> {
        if let Some(created_flow_step_inputs_by_node) = &branch.created_flow_step_inputs_by_node {
            let flow_step_ids: Set<Uuid> = created_flow_step_inputs_by_node.keys().cloned().collect();
            let flow_steps = UpdateNodeIdsFlowStep::find_by_branch_id_and_ids(db_session, branch.id, &flow_step_ids)
                .await
                .try_collect()
                .await?;

            return Ok(Some(flow_steps.into_iter().map(|fs| (fs.id, fs)).collect()));
        }

        Ok(None)
    }

    // Returns original flow steps
    pub async fn deleted_fs_inputs_flow_steps(
        db_session: &CachingSession,
        branch: &Branch,
    ) -> Result<Option<Vec<UpdateInputIdsFlowStep>>, NodecosmosError> {
        if let Some(deleted_flow_step_inputs_by_node) = &branch.deleted_flow_step_inputs_by_node {
            let flow_step_ids: Set<Uuid> = deleted_flow_step_inputs_by_node.keys().cloned().collect();
            let flow_steps =
                UpdateInputIdsFlowStep::find_by_branch_id_and_ids(db_session, branch.original_id(), &flow_step_ids)
                    .await
                    .try_collect()
                    .await?;

            return Ok(Some(Self::filter_out_deleted_flow_steps(branch, flow_steps)));
        }

        Ok(None)
    }

    pub async fn new(db_session: &CachingSession, branch: &Branch) -> Result<Self, NodecosmosError> {
        let restored_flow_steps = Self::restored_flow_steps(db_session, branch).await?;
        let created_flow_steps = Self::created_flow_steps(db_session, branch).await?;
        let deleted_flow_steps = Self::deleted_flow_steps(db_session, branch).await?;
        let created_fs_nodes_flow_steps = Self::created_fs_nodes_flow_steps(db_session, branch).await?;
        let branched_created_fs_nodes_flow_steps =
            Self::branched_created_fs_nodes_flow_steps(db_session, branch).await?;
        let deleted_fs_nodes_flow_steps = Self::deleted_fs_nodes_flow_steps(db_session, branch).await?;
        let created_fs_inputs_flow_steps = Self::created_fs_inputs_flow_steps(db_session, branch).await?;
        let branched_created_fs_inputs_flow_steps =
            Self::branched_created_fs_inputs_flow_steps(db_session, branch).await?;
        let deleted_fs_inputs_flow_steps = Self::deleted_fs_inputs_flow_steps(db_session, branch).await?;

        Ok(Self {
            restored_flow_steps,
            created_flow_steps,
            deleted_flow_steps,
            created_fs_nodes_flow_steps,
            branched_created_fs_nodes_flow_steps,
            deleted_fs_nodes_flow_steps,
            created_fs_inputs_flow_steps,
            branched_created_fs_inputs_flow_steps,
            deleted_fs_inputs_flow_steps,
            // Delta fields
            added_node_ids_by_flow_step: None,
            removed_node_ids_by_flow_step: None,
            added_input_ids_by_flow_step: None,
            removed_input_ids_by_flow_step: None,
            added_output_ids_by_flow_step: None,
            removed_output_ids_by_flow_step: None,
        })
    }

    pub async fn delete_inserted_flow_steps(
        data: &RequestData,
        merge_flow_steps: &mut Option<Vec<FlowStep>>,
    ) -> Result<(), NodecosmosError> {
        if let Some(merge_flow_steps) = merge_flow_steps {
            for merge_flow_step in merge_flow_steps {
                merge_flow_step.set_merge_context();
                merge_flow_step.set_original_id();
                merge_flow_step.delete_cb(data).execute(data.db_session()).await?;
            }
        }

        Ok(())
    }

    pub async fn restore_flow_steps(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        if let Some(flow_steps) = &mut self.restored_flow_steps {
            for flow_step in flow_steps {
                flow_step.set_merge_context();
                flow_step.set_original_id();
                flow_step
                    .insert_cb(data)
                    .execute(data.db_session())
                    .await
                    .context("Error restoring flow steps")?;
            }
        }

        Ok(())
    }

    pub async fn undo_restore_flow_steps(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        Self::delete_inserted_flow_steps(data, &mut self.restored_flow_steps)
            .await
            .context("Error undoing restore flow steps")?;

        Ok(())
    }

    pub async fn create_flow_steps(&mut self, data: &RequestData, branch: &Branch) -> Result<(), NodecosmosError> {
        let kept_flow_steps = branch.kept_flow_steps.as_ref();

        if let Some(flow_steps) = &mut self.created_flow_steps {
            for flow_step in flow_steps {
                // Check if flow step is kept despite having conflicts, if so, we need to increment step index, and
                // update branch version with new step index.
                if kept_flow_steps.is_some_and(|kfs| kfs.contains(&flow_step.id)) {
                    // as we can not update clustering keys,
                    // we need to delete and insert again with new step index
                    flow_step.delete().execute(data.db_session()).await?;

                    // increment step index
                    flow_step.step_index.increment_fraction();

                    // insert branched with new step index
                    flow_step.insert().execute(data.db_session()).await?;
                }

                flow_step.set_merge_context();
                flow_step.set_original_id();

                flow_step
                    .insert_cb(data)
                    .execute(data.db_session())
                    .await
                    .context("Error restoring flow steps")?;
            }
        }

        Ok(())
    }

    pub async fn undo_create_flow_steps(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        Self::delete_inserted_flow_steps(data, &mut self.created_flow_steps)
            .await
            .context("Error undoing create flow steps")?;

        Ok(())
    }

    pub async fn delete_flow_steps(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        if let Some(deleted_flow_steps) = &mut self.deleted_flow_steps {
            for deleted_flow_step in deleted_flow_steps {
                deleted_flow_step.set_merge_context();
                deleted_flow_step
                    .delete_cb(data)
                    .execute(data.db_session())
                    .await
                    .context("Error deleting flow step")?;
            }
        }

        Ok(())
    }

    pub async fn undo_delete_flow_steps(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        if let Some(deleted_flow_steps) = &mut self.deleted_flow_steps {
            for deleted_flow_step in deleted_flow_steps {
                deleted_flow_step.set_merge_context();
                deleted_flow_step
                    .insert_cb(data)
                    .execute(data.db_session())
                    .await
                    .context("Error undoing delete flow step")?;
            }
        }

        Ok(())
    }

    pub async fn create_flow_step_nodes(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        if let Some(created_fs_nodes_flow_steps) = &mut self.created_fs_nodes_flow_steps {
            for original_flow_step in created_fs_nodes_flow_steps {
                let branch_fs = self
                    .branched_created_fs_nodes_flow_steps
                    .as_ref()
                    .and_then(|m| m.get(&original_flow_step.id));

                if let Some(branch_fs) = branch_fs {
                    // Calculate added delta so we don't add nodes that may be already added outside
                    // of the current branch.
                    let mut added_delta = vec![];

                    if let Some(node_ids) = branch_fs.node_ids.as_ref() {
                        node_ids.iter().for_each(|node_id| {
                            if let Some(original_node_ids) = original_flow_step.node_ids.as_ref() {
                                if !original_node_ids.contains(node_id) {
                                    added_delta.push(*node_id);
                                }
                            } else {
                                added_delta.push(*node_id);
                            }
                        });
                    };

                    if added_delta.is_empty() {
                        continue;
                    }

                    // run update
                    original_flow_step.append_nodes(&added_delta);
                    original_flow_step.set_merge_context();
                    original_flow_step
                        .update_cb(data)
                        .execute(data.db_session())
                        .await
                        .context("Error creating flow step nodes")?;

                    // save added delta for undo
                    self.added_node_ids_by_flow_step
                        .get_or_insert_with(HashMap::default)
                        .insert(original_flow_step.id, added_delta);
                }
            }
        }

        Ok(())
    }

    pub async fn undo_create_flow_step_nodes(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        if let (Some(created_fs_nodes_flow_steps), Some(added_node_ids_by_flow_step)) = (
            &mut self.created_fs_nodes_flow_steps,
            self.added_node_ids_by_flow_step.as_ref(),
        ) {
            for original_flow_step in created_fs_nodes_flow_steps {
                if let Some(node_ids) = added_node_ids_by_flow_step.get(&original_flow_step.id) {
                    original_flow_step.remove_nodes(node_ids);
                    original_flow_step.set_merge_context();
                    original_flow_step
                        .update_cb(data)
                        .execute(data.db_session())
                        .await
                        .context("Error undoing create flow step nodes")?;
                }
            }
        }

        Ok(())
    }

    pub async fn delete_flow_step_nodes(&mut self, data: &RequestData, branch: &Branch) -> Result<(), NodecosmosError> {
        if let (Some(deleted_fs_nodes_flow_steps), Some(deleted_flow_step_nodes)) =
            (&mut self.deleted_fs_nodes_flow_steps, &branch.deleted_flow_step_nodes)
        {
            for original_flow_step in deleted_fs_nodes_flow_steps {
                let deleted_node_ids = deleted_flow_step_nodes.get(&original_flow_step.id);
                if let Some(deleted_node_ids) = deleted_node_ids {
                    // Check and reload original flow step if it's present in created flow step nodes so we don't
                    // overwrite changes made in this merge. This is a temporary solution as we are loading flow steps
                    // per merge step. In the future, we should load all flow steps at same place and get them
                    // from there by using branch refs.
                    if branch
                        .created_flow_step_nodes
                        .as_ref()
                        .is_some_and(|cfs| cfs.contains_key(&original_flow_step.id))
                    {
                        original_flow_step.reload(data.db_session()).await?;
                    }

                    // Calculate removed nodes so we don't remove nodes that may be already removed outside of the
                    // current branch.
                    let mut removed_delta = vec![];
                    deleted_node_ids.iter().for_each(|node_id| {
                        if let Some(original_node_ids) = original_flow_step.node_ids.as_ref() {
                            if original_node_ids.contains(node_id) {
                                removed_delta.push(*node_id);
                            }
                        }
                    });

                    if removed_delta.is_empty() {
                        continue;
                    }

                    // run update
                    original_flow_step.remove_nodes(&removed_delta);
                    original_flow_step.set_merge_context();
                    original_flow_step
                        .update_cb(data)
                        .execute(data.db_session())
                        .await
                        .context("Error deleting flow step nodes")?;

                    // save removed delta for undo
                    self.removed_node_ids_by_flow_step
                        .get_or_insert_with(HashMap::default)
                        .insert(original_flow_step.id, removed_delta);
                }
            }
        }

        Ok(())
    }

    pub async fn undo_delete_flow_step_nodes(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        if let Some(deleted_fs_nodes_flow_steps) = &mut self.deleted_fs_nodes_flow_steps {
            for original_flow_step in deleted_fs_nodes_flow_steps {
                if let Some(removed_node_ids) = self
                    .removed_node_ids_by_flow_step
                    .as_ref()
                    .and_then(|removed_node_ids_by_flow_step| removed_node_ids_by_flow_step.get(&original_flow_step.id))
                {
                    original_flow_step.append_nodes(removed_node_ids);
                    original_flow_step.set_merge_context();
                    original_flow_step
                        .update_cb(data)
                        .execute(data.db_session())
                        .await
                        .context("Error undoing delete flow step nodes")?;
                }
            }
        }

        Ok(())
    }

    pub async fn create_inputs(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        if let Some(created_fs_inputs_flow_steps) = &mut self.created_fs_inputs_flow_steps {
            for original_flow_step in created_fs_inputs_flow_steps {
                let branched_fs = self
                    .branched_created_fs_inputs_flow_steps
                    .as_ref()
                    .and_then(|m| m.get(&original_flow_step.id));

                if let Some(branched_fs) = branched_fs {
                    // Calculate added delta so we don't add inputs that may be already added outside of the current branch.
                    let mut added_inputs_delta = HashMap::default();
                    if let Some(input_ids_by_node_id) = branched_fs.input_ids_by_node_id.as_ref() {
                        input_ids_by_node_id.iter().for_each(|(node_id, input_ids)| {
                            let original_node_inputs = original_flow_step
                                .input_ids_by_node_id
                                .as_ref()
                                .and_then(|m| m.get(node_id));

                            let added_delta = input_ids
                                .iter()
                                .filter_map(|input_id| {
                                    if original_node_inputs.is_none()
                                        || !original_node_inputs.is_some_and(|inputs| inputs.contains(input_id))
                                    {
                                        Some(*input_id)
                                    } else {
                                        None
                                    }
                                })
                                .collect();

                            added_inputs_delta.insert(*node_id, added_delta);
                        });
                    }

                    if added_inputs_delta.is_empty() {
                        continue;
                    }

                    // run update
                    original_flow_step.append_inputs(&added_inputs_delta);
                    original_flow_step.set_merge_context();
                    original_flow_step
                        .update_cb(data)
                        .execute(data.db_session())
                        .await
                        .context("Error creating inputs")?;

                    // save added delta for undo
                    self.added_input_ids_by_flow_step
                        .get_or_insert_with(HashMap::default)
                        .insert(original_flow_step.id, added_inputs_delta);
                }
            }
        }

        Ok(())
    }

    pub async fn undo_create_inputs(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        if let Some(created_fs_inputs_flow_steps) = &mut self.created_fs_inputs_flow_steps {
            for original_flow_step in created_fs_inputs_flow_steps {
                if let Some(added_input_ids_by_node_id) = self
                    .added_input_ids_by_flow_step
                    .as_ref()
                    .and_then(|added_input_ids_by_flow_step| added_input_ids_by_flow_step.get(&original_flow_step.id))
                {
                    original_flow_step.remove_inputs(added_input_ids_by_node_id);
                    original_flow_step.set_merge_context();
                    original_flow_step
                        .update_cb(data)
                        .execute(data.db_session())
                        .await
                        .context("Error undoing create inputs")?;
                }
            }
        }

        Ok(())
    }

    pub async fn delete_inputs(&mut self, data: &RequestData, branch: &Branch) -> Result<(), NodecosmosError> {
        if let (Some(deleted_fs_inputs_flow_steps), Some(deleted_flow_step_inputs_by_node)) = (
            &mut self.deleted_fs_inputs_flow_steps,
            &branch.deleted_flow_step_inputs_by_node,
        ) {
            for original_flow_step in deleted_fs_inputs_flow_steps {
                let deleted_inputs_by_node = deleted_flow_step_inputs_by_node.get(&original_flow_step.id);
                // Check and reload original flow step if it's present in created flow step inputs so we don't
                // overwrite changes made in this merge.
                if let Some(deleted_inputs_by_node) = deleted_inputs_by_node {
                    if branch
                        .created_flow_step_inputs_by_node
                        .as_ref()
                        .is_some_and(|cfs| cfs.contains_key(&original_flow_step.id))
                    {
                        original_flow_step.reload(data.db_session()).await?;
                    }

                    // Calculate removed delta so we don't remove inputs that may be already removed outside of the
                    // current branch.
                    let mut deleted_inputs_delta = HashMap::default();
                    deleted_inputs_by_node.iter().for_each(|(node_id, input_ids)| {
                        if let Some(original_input_ids) = original_flow_step
                            .input_ids_by_node_id
                            .as_ref()
                            .and_then(|m| m.get(node_id))
                        {
                            let created_node_inputs = branch
                                .created_flow_step_inputs_by_node
                                .as_ref()
                                .and_then(|m| m.get(&original_flow_step.id))
                                .and_then(|m| m.get(node_id));
                            let mut removed_delta = vec![];
                            input_ids.iter().for_each(|input_id| {
                                // skip if it's in created inputs
                                if created_node_inputs.is_some_and(|ios| ios.contains(input_id)) {
                                    return;
                                }

                                if original_input_ids.contains(input_id) {
                                    removed_delta.push(*input_id);
                                }
                            });

                            deleted_inputs_delta.insert(*node_id, removed_delta);
                        }
                    });

                    if deleted_inputs_delta.is_empty() {
                        continue;
                    }

                    // run update
                    original_flow_step.remove_inputs(&deleted_inputs_delta);
                    original_flow_step.set_merge_context();
                    original_flow_step
                        .update_cb(data)
                        .execute(data.db_session())
                        .await
                        .context("Error deleting inputs")?;

                    self.removed_input_ids_by_flow_step
                        .get_or_insert_with(HashMap::default)
                        .insert(original_flow_step.id, deleted_inputs_delta);
                }
            }
        }

        Ok(())
    }

    pub async fn undo_delete_inputs(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        if let Some(deleted_fs_inputs_flow_steps) = &mut self.deleted_fs_inputs_flow_steps {
            for original_flow_step in deleted_fs_inputs_flow_steps {
                if let Some(removed_input_ids_by_node) =
                    self.removed_input_ids_by_flow_step
                        .as_ref()
                        .and_then(|removed_input_ids_by_flow_step| {
                            removed_input_ids_by_flow_step.get(&original_flow_step.id)
                        })
                {
                    original_flow_step.append_inputs(removed_input_ids_by_node);

                    original_flow_step.set_merge_context();
                    original_flow_step
                        .update_cb(data)
                        .execute(data.db_session())
                        .await
                        .context("Error undoing delete inputs")?;
                }
            }
        }

        Ok(())
    }
}
