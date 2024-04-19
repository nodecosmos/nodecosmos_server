use std::collections::HashMap;

use anyhow::Context;
use charybdis::operations::{DeleteWithCallbacks, InsertWithCallbacks, UpdateWithCallbacks};
use charybdis::types::{Set, Uuid};
use scylla::CachingSession;
use serde::{Deserialize, Serialize};

use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::branch::Branch;
use crate::models::description::{find_description, Description};
use crate::models::flow_step::{FlowStep, UpdateInputIdsFlowStep, UpdateNodeIdsFlowStep, UpdateOutputIdsFlowStep};
use crate::models::traits::{Branchable, FindForBranchMerge, GroupByObjectId, IncrementFraction, ObjectType, Reload};
use crate::models::traits::{ModelContext, PluckFromStream};
use crate::models::udts::TextChange;

#[derive(Serialize, Deserialize)]
pub struct MergeFlowSteps {
    pub restored_flow_steps: Option<Vec<FlowStep>>,
    pub created_flow_steps: Option<Vec<FlowStep>>,
    pub deleted_flow_steps: Option<Vec<FlowStep>>,
    pub edited_flow_step_descriptions: Option<Vec<Description>>,
    pub created_fs_nodes_flow_steps: Option<Vec<UpdateNodeIdsFlowStep>>,
    pub branched_created_fs_nodes_flow_steps: Option<HashMap<Uuid, UpdateNodeIdsFlowStep>>,
    pub deleted_fs_nodes_flow_steps: Option<Vec<UpdateNodeIdsFlowStep>>,
    pub created_fs_inputs_flow_steps: Option<Vec<UpdateInputIdsFlowStep>>,
    pub branched_created_fs_inputs_flow_steps: Option<HashMap<Uuid, UpdateNodeIdsFlowStep>>,
    pub deleted_fs_inputs_flow_steps: Option<Vec<UpdateInputIdsFlowStep>>,
    pub created_fs_outputs_flow_steps: Option<Vec<UpdateOutputIdsFlowStep>>,
    pub branched_created_fs_outputs_flow_steps: Option<HashMap<Uuid, UpdateNodeIdsFlowStep>>,
    pub deleted_fs_outputs_flow_steps: Option<Vec<UpdateOutputIdsFlowStep>>,
    pub original_flow_step_descriptions: Option<HashMap<Uuid, Description>>,
    // Delta fields that are calculated during merge
    pub added_node_ids_by_flow_step: Option<HashMap<Uuid, Vec<Uuid>>>,
    pub removed_node_ids_by_flow_step: Option<HashMap<Uuid, Vec<Uuid>>>,
    pub added_input_ids_by_flow_step: Option<HashMap<Uuid, HashMap<Uuid, Vec<Uuid>>>>,
    pub removed_input_ids_by_flow_step: Option<HashMap<Uuid, HashMap<Uuid, Vec<Uuid>>>>,
    pub added_output_ids_by_flow_step: Option<HashMap<Uuid, HashMap<Uuid, Vec<Uuid>>>>,
    pub removed_output_ids_by_flow_step: Option<HashMap<Uuid, HashMap<Uuid, Vec<Uuid>>>>,
}

impl MergeFlowSteps {
    async fn original_flow_step_descriptions(
        db_session: &CachingSession,
        branch: &Branch,
    ) -> Result<Option<HashMap<Uuid, Description>>, NodecosmosError> {
        if let Some(ids) = &branch.edited_description_flow_steps {
            let ios_by_id = find_description!("object_id IN ? AND branch_id IN ?", (ids, ids))
                .execute(db_session)
                .await?
                .group_by_object_id()
                .await?;

            return Ok(Some(ios_by_id));
        }

        Ok(None)
    }

    pub async fn edited_flow_step_descriptions(
        branch: &Branch,
        db_session: &CachingSession,
    ) -> Result<Option<Vec<Description>>, NodecosmosError> {
        if let Some(edited_description_flow_step_ids) = &branch.edited_description_flow_steps {
            let descriptions = find_description!(
                "branch_id = ? AND object_id IN ?",
                (branch.id, edited_description_flow_step_ids)
            )
            .execute(db_session)
            .await?
            .try_collect()
            .await?;

            return Ok(Some(
                branch
                    .map_original_objects(ObjectType::FlowStep, descriptions)
                    .collect(),
            ));
        }

        Ok(None)
    }

    pub async fn restored_flow_steps(
        branch: &Branch,
        db_session: &CachingSession,
    ) -> Result<Option<Vec<FlowStep>>, NodecosmosError> {
        if let (Some(edited_workflow_node_ids), Some(restored_flow_step_ids)) =
            (&branch.edited_workflow_nodes, &branch.restored_flow_steps)
        {
            let already_restored_ids =
                FlowStep::find_original_by_ids(db_session, edited_workflow_node_ids, restored_flow_step_ids)
                    .await?
                    .pluck_id_set()
                    .await?;

            let mut flow_steps = FlowStep::find_by_node_ids_and_branch_id_and_ids(
                db_session,
                edited_workflow_node_ids,
                branch.id,
                restored_flow_step_ids,
            )
            .await?;

            flow_steps.retain(|flow_step| !already_restored_ids.contains(&flow_step.id));

            return Ok(Some(flow_steps));
        }

        Ok(None)
    }

    pub async fn created_flow_steps(
        branch: &Branch,
        db_session: &CachingSession,
    ) -> Result<Option<Vec<FlowStep>>, NodecosmosError> {
        if let (Some(edited_workflow_node_ids), Some(created_flow_step_ids)) =
            (&branch.edited_workflow_nodes, &branch.created_flow_steps)
        {
            let flow_steps = FlowStep::find_by_node_ids_and_branch_id_and_ids(
                db_session,
                edited_workflow_node_ids,
                branch.id,
                created_flow_step_ids,
            )
            .await?;

            return Ok(Some(flow_steps));
        }

        Ok(None)
    }

    pub async fn deleted_flow_steps(
        branch: &Branch,
        db_session: &CachingSession,
    ) -> Result<Option<Vec<FlowStep>>, NodecosmosError> {
        if let (Some(edited_workflow_node_ids), Some(deleted_flow_step_ids)) =
            (&branch.edited_workflow_nodes, &branch.deleted_flow_steps)
        {
            let flow_steps =
                FlowStep::find_original_by_ids(db_session, edited_workflow_node_ids, deleted_flow_step_ids)
                    .await?
                    .try_collect()
                    .await?;

            return Ok(Some(flow_steps));
        }

        Ok(None)
    }

    // Returns original flow steps
    pub async fn created_fs_nodes_flow_steps(
        branch: &Branch,
        db_session: &CachingSession,
    ) -> Result<Option<Vec<UpdateNodeIdsFlowStep>>, NodecosmosError> {
        if let (Some(edited_workflow_node_ids), Some(created_flow_step_nodes)) =
            (&branch.edited_workflow_nodes, &branch.created_flow_step_nodes)
        {
            let flow_step_ids: Set<Uuid> = created_flow_step_nodes.keys().cloned().collect();
            let flow_steps =
                UpdateNodeIdsFlowStep::find_original_by_ids(db_session, edited_workflow_node_ids, &flow_step_ids)
                    .await?
                    .try_collect()
                    .await?;

            return Ok(Some(flow_steps));
        }

        Ok(None)
    }

    pub async fn branched_created_fs_nodes_flow_steps(
        branch: &Branch,
        db_session: &CachingSession,
    ) -> Result<Option<HashMap<Uuid, UpdateNodeIdsFlowStep>>, NodecosmosError> {
        if let (Some(edited_workflow_node_ids), Some(created_flow_step_nodes)) =
            (&branch.edited_workflow_nodes, &branch.created_flow_step_nodes)
        {
            let flow_step_ids: Set<Uuid> = created_flow_step_nodes.keys().cloned().collect();
            let flow_steps = UpdateNodeIdsFlowStep::find_by_node_ids_and_branch_id_and_ids(
                db_session,
                edited_workflow_node_ids,
                branch.id,
                &flow_step_ids,
            )
            .await?;

            return Ok(Some(flow_steps.into_iter().map(|fs| (fs.id, fs)).collect()));
        }

        Ok(None)
    }

    // Returns original flow steps
    pub async fn deleted_fs_nodes_flow_steps(
        branch: &Branch,
        db_session: &CachingSession,
    ) -> Result<Option<Vec<UpdateNodeIdsFlowStep>>, NodecosmosError> {
        if let (Some(edited_workflow_node_ids), Some(deleted_flow_step_nodes)) =
            (&branch.edited_workflow_nodes, &branch.deleted_flow_step_nodes)
        {
            let flow_step_ids: Set<Uuid> = deleted_flow_step_nodes.keys().cloned().collect();
            let flow_steps =
                UpdateNodeIdsFlowStep::find_original_by_ids(db_session, edited_workflow_node_ids, &flow_step_ids)
                    .await?
                    .try_collect()
                    .await?;

            return Ok(Some(flow_steps));
        }

        Ok(None)
    }

    // Returns original flow steps
    pub async fn created_fs_inputs_flow_steps(
        branch: &Branch,
        db_session: &CachingSession,
    ) -> Result<Option<Vec<UpdateInputIdsFlowStep>>, NodecosmosError> {
        if let (Some(edited_workflow_node_ids), Some(created_flow_step_inputs_by_node)) =
            (&branch.edited_workflow_nodes, &branch.created_flow_step_inputs_by_node)
        {
            let flow_step_ids: Set<Uuid> = created_flow_step_inputs_by_node.keys().cloned().collect();
            let flow_steps =
                UpdateInputIdsFlowStep::find_original_by_ids(db_session, edited_workflow_node_ids, &flow_step_ids)
                    .await?
                    .try_collect()
                    .await?;

            return Ok(Some(flow_steps));
        }

        Ok(None)
    }

    pub async fn branched_created_fs_inputs_flow_steps(
        branch: &Branch,
        db_session: &CachingSession,
    ) -> Result<Option<HashMap<Uuid, UpdateNodeIdsFlowStep>>, NodecosmosError> {
        if let (Some(edited_workflow_node_ids), Some(created_flow_step_inputs_by_node)) =
            (&branch.edited_workflow_nodes, &branch.created_flow_step_inputs_by_node)
        {
            let flow_step_ids: Set<Uuid> = created_flow_step_inputs_by_node.keys().cloned().collect();
            let flow_steps = UpdateNodeIdsFlowStep::find_by_node_ids_and_branch_id_and_ids(
                db_session,
                edited_workflow_node_ids,
                branch.id,
                &flow_step_ids,
            )
            .await?;

            return Ok(Some(flow_steps.into_iter().map(|fs| (fs.id, fs)).collect()));
        }

        Ok(None)
    }

    // Returns original flow steps
    pub async fn deleted_fs_inputs_flow_steps(
        branch: &Branch,
        db_session: &CachingSession,
    ) -> Result<Option<Vec<UpdateInputIdsFlowStep>>, NodecosmosError> {
        if let (Some(edited_workflow_node_ids), Some(deleted_flow_step_inputs_by_node)) =
            (&branch.edited_workflow_nodes, &branch.deleted_flow_step_inputs_by_node)
        {
            let flow_step_ids: Set<Uuid> = deleted_flow_step_inputs_by_node.keys().cloned().collect();
            let flow_steps =
                UpdateInputIdsFlowStep::find_original_by_ids(db_session, edited_workflow_node_ids, &flow_step_ids)
                    .await?
                    .try_collect()
                    .await?;

            return Ok(Some(flow_steps));
        }

        Ok(None)
    }

    // Returns original flow steps
    pub async fn created_fs_outputs_flow_steps(
        branch: &Branch,
        db_session: &CachingSession,
    ) -> Result<Option<Vec<UpdateOutputIdsFlowStep>>, NodecosmosError> {
        if let (Some(edited_workflow_node_ids), Some(created_flow_step_outputs_by_node)) =
            (&branch.edited_workflow_nodes, &branch.created_flow_step_outputs_by_node)
        {
            let flow_step_ids: Set<Uuid> = created_flow_step_outputs_by_node.keys().cloned().collect();
            let flow_steps =
                UpdateOutputIdsFlowStep::find_original_by_ids(db_session, edited_workflow_node_ids, &flow_step_ids)
                    .await?
                    .try_collect()
                    .await?;

            return Ok(Some(flow_steps));
        }

        Ok(None)
    }

    pub async fn branched_created_fs_outputs_flow_steps(
        branch: &Branch,
        db_session: &CachingSession,
    ) -> Result<Option<HashMap<Uuid, UpdateNodeIdsFlowStep>>, NodecosmosError> {
        if let (Some(edited_workflow_node_ids), Some(created_flow_step_outputs_by_node)) =
            (&branch.edited_workflow_nodes, &branch.created_flow_step_outputs_by_node)
        {
            let flow_step_ids: Set<Uuid> = created_flow_step_outputs_by_node.keys().cloned().collect();
            let flow_steps = UpdateNodeIdsFlowStep::find_by_node_ids_and_branch_id_and_ids(
                db_session,
                edited_workflow_node_ids,
                branch.id,
                &flow_step_ids,
            )
            .await?;

            return Ok(Some(flow_steps.into_iter().map(|fs| (fs.id, fs)).collect()));
        }

        Ok(None)
    }

    // Returns original flow steps
    pub async fn deleted_fs_outputs_flow_steps(
        branch: &Branch,
        db_session: &CachingSession,
    ) -> Result<Option<Vec<UpdateOutputIdsFlowStep>>, NodecosmosError> {
        if let (Some(edited_workflow_node_ids), Some(deleted_flow_step_outputs_by_node)) =
            (&branch.edited_workflow_nodes, &branch.deleted_flow_step_outputs_by_node)
        {
            let flow_step_ids: Set<Uuid> = deleted_flow_step_outputs_by_node.keys().cloned().collect();
            let flow_steps =
                UpdateOutputIdsFlowStep::find_original_by_ids(db_session, edited_workflow_node_ids, &flow_step_ids)
                    .await?
                    .try_collect()
                    .await?;

            return Ok(Some(flow_steps));
        }

        Ok(None)
    }

    pub async fn new(branch: &Branch, data: &RequestData) -> Result<Self, NodecosmosError> {
        let restored_flow_steps = Self::restored_flow_steps(&branch, data.db_session()).await?;
        let created_flow_steps = Self::created_flow_steps(&branch, data.db_session()).await?;
        let deleted_flow_steps = Self::deleted_flow_steps(&branch, data.db_session()).await?;
        let edited_flow_step_descriptions = Self::edited_flow_step_descriptions(&branch, data.db_session()).await?;
        let original_flow_step_descriptions = Self::original_flow_step_descriptions(data.db_session(), &branch).await?;
        let created_fs_nodes_flow_steps = Self::created_fs_nodes_flow_steps(&branch, data.db_session()).await?;
        let branched_created_fs_nodes_flow_steps =
            Self::branched_created_fs_nodes_flow_steps(&branch, data.db_session()).await?;
        let deleted_fs_nodes_flow_steps = Self::deleted_fs_nodes_flow_steps(&branch, data.db_session()).await?;
        let created_fs_inputs_flow_steps = Self::created_fs_inputs_flow_steps(&branch, data.db_session()).await?;
        let branched_created_fs_inputs_flow_steps =
            Self::branched_created_fs_inputs_flow_steps(&branch, data.db_session()).await?;
        let deleted_fs_inputs_flow_steps = Self::deleted_fs_inputs_flow_steps(&branch, data.db_session()).await?;
        let created_fs_outputs_flow_steps = Self::created_fs_outputs_flow_steps(&branch, data.db_session()).await?;
        let branched_created_fs_outputs_flow_steps =
            Self::branched_created_fs_outputs_flow_steps(&branch, data.db_session()).await?;
        let deleted_fs_outputs_flow_steps = Self::deleted_fs_outputs_flow_steps(&branch, data.db_session()).await?;

        Ok(Self {
            restored_flow_steps,
            created_flow_steps,
            deleted_flow_steps,
            edited_flow_step_descriptions,
            original_flow_step_descriptions,
            created_fs_nodes_flow_steps,
            branched_created_fs_nodes_flow_steps,
            deleted_fs_nodes_flow_steps,
            created_fs_inputs_flow_steps,
            branched_created_fs_inputs_flow_steps,
            deleted_fs_inputs_flow_steps,
            created_fs_outputs_flow_steps,
            branched_created_fs_outputs_flow_steps,
            deleted_fs_outputs_flow_steps,
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
                if kept_flow_steps.is_some_and(|kfs| kfs.contains(&flow_step.id)) {
                    flow_step.flow_index.increment_fraction();
                    println!("{}", flow_step.flow_index);
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
                    // Calculate added delta so we don't add nodes that may be already added outside of the current branch.
                    let mut added_delta = vec![];

                    if let Some(node_ids) = branch_fs.node_ids.as_ref() {
                        node_ids.iter().for_each(|node_id| {
                            if let Some(original_node_ids) = original_flow_step.node_ids.as_ref() {
                                if !original_node_ids.contains(node_id) {
                                    added_delta.push(*node_id);
                                }
                            }
                        });
                    };

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
                    original_flow_step.remove_nodes(&node_ids);
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
                    .map_or(None, |removed_node_ids_by_flow_step| {
                        removed_node_ids_by_flow_step.get(&original_flow_step.id)
                    })
                {
                    original_flow_step.append_nodes(&removed_node_ids);
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
                                .into_iter()
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
                    .map_or(None, |added_input_ids_by_flow_step| {
                        added_input_ids_by_flow_step.get(&original_flow_step.id)
                    })
                {
                    original_flow_step.remove_inputs(&added_input_ids_by_node_id);
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
                            let mut removed_delta = vec![];
                            input_ids.iter().for_each(|input_id| {
                                if original_input_ids.contains(input_id) {
                                    removed_delta.push(*input_id);
                                }
                            });

                            deleted_inputs_delta.insert(*node_id, removed_delta);
                        }
                    });

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
                if let Some(removed_input_ids_by_node) = self
                    .removed_input_ids_by_flow_step
                    .as_ref()
                    .map_or(None, |removed_input_ids_by_flow_step| {
                        removed_input_ids_by_flow_step.get(&original_flow_step.id)
                    })
                {
                    original_flow_step.append_inputs(&removed_input_ids_by_node);

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

    pub async fn create_outputs(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        if let Some(created_fs_outputs_flow_steps) = &mut self.created_fs_outputs_flow_steps {
            for original_flow_step in created_fs_outputs_flow_steps {
                let branched_fs = self
                    .branched_created_fs_outputs_flow_steps
                    .as_ref()
                    .and_then(|m| m.get(&original_flow_step.id));

                // Calculate added delta so we don't add outputs that may be already added outside of the current branch.

                if let Some(branched_fs) = branched_fs {
                    let mut added_outputs_delta = HashMap::default();
                    if let Some(output_ids_by_node_id) = branched_fs.output_ids_by_node_id.as_ref() {
                        output_ids_by_node_id.iter().for_each(|(node_id, output_ids)| {
                            let original_node_outputs = original_flow_step
                                .output_ids_by_node_id
                                .as_ref()
                                .and_then(|m| m.get(node_id));

                            let added_delta = output_ids
                                .into_iter()
                                .filter_map(|output_id| {
                                    if original_node_outputs.is_none()
                                        || !original_node_outputs.is_some_and(|outputs| outputs.contains(output_id))
                                    {
                                        Some(*output_id)
                                    } else {
                                        None
                                    }
                                })
                                .collect();

                            added_outputs_delta.insert(*node_id, added_delta);
                        });
                    }

                    // run update
                    original_flow_step.append_outputs(&added_outputs_delta);
                    original_flow_step.set_merge_context();
                    original_flow_step
                        .update_cb(data)
                        .execute(data.db_session())
                        .await
                        .context("Error creating outputs")?;

                    // save added delta for undo
                    self.added_output_ids_by_flow_step
                        .get_or_insert_with(HashMap::default)
                        .insert(original_flow_step.id, added_outputs_delta);
                }
            }
        }

        Ok(())
    }

    pub async fn undo_create_outputs(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        if let Some(created_fs_outputs_flow_steps) = &mut self.created_fs_outputs_flow_steps {
            for original_flow_step in created_fs_outputs_flow_steps {
                if let Some(added_output_ids_by_node_id) = self
                    .added_output_ids_by_flow_step
                    .as_ref()
                    .map_or(None, |added_output_ids_by_flow_step| {
                        added_output_ids_by_flow_step.get(&original_flow_step.id)
                    })
                {
                    original_flow_step.remove_outputs(&added_output_ids_by_node_id);

                    original_flow_step.set_merge_context();
                    original_flow_step
                        .update_cb(data)
                        .execute(data.db_session())
                        .await
                        .context("Error undoing create outputs")?;
                }
            }
        }

        Ok(())
    }

    pub async fn delete_outputs(&mut self, data: &RequestData, branch: &Branch) -> Result<(), NodecosmosError> {
        if let (Some(deleted_fs_outputs_flow_steps), Some(deleted_flow_step_outputs_by_node)) = (
            &mut self.deleted_fs_outputs_flow_steps,
            &branch.deleted_flow_step_outputs_by_node,
        ) {
            for original_flow_step in deleted_fs_outputs_flow_steps {
                let deleted_outputs_by_node = deleted_flow_step_outputs_by_node.get(&original_flow_step.id);
                // Check and reload original flow step if it's present in created flow step outputs so we don't
                // overwrite changes made in this merge.
                if let Some(deleted_outputs_by_node) = deleted_outputs_by_node {
                    if branch
                        .created_flow_step_outputs_by_node
                        .as_ref()
                        .is_some_and(|cfs| cfs.contains_key(&original_flow_step.id))
                    {
                        original_flow_step.reload(data.db_session()).await?;
                    }

                    // Calculate removed delta so we don't remove outputs that may be already removed outside of the
                    // current branch.
                    let mut deleted_outputs_delta = HashMap::default();
                    deleted_outputs_by_node.iter().for_each(|(node_id, output_ids)| {
                        if let Some(original_output_ids) = original_flow_step
                            .output_ids_by_node_id
                            .as_ref()
                            .and_then(|m| m.get(node_id))
                        {
                            let mut removed_delta = vec![];
                            output_ids.iter().for_each(|output_id| {
                                if original_output_ids.contains(output_id) {
                                    removed_delta.push(*output_id);
                                }
                            });

                            deleted_outputs_delta.insert(*node_id, removed_delta);
                        }
                    });

                    original_flow_step.remove_outputs(&deleted_outputs_delta);
                    original_flow_step.set_merge_context();
                    original_flow_step
                        .update_cb(data)
                        .execute(data.db_session())
                        .await
                        .context("Error deleting outputs")?;

                    self.removed_output_ids_by_flow_step
                        .get_or_insert_with(HashMap::default)
                        .insert(original_flow_step.id, deleted_outputs_delta);
                }
            }
        }

        Ok(())
    }

    pub async fn undo_delete_outputs(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        if let Some(deleted_fs_outputs_flow_steps) = &mut self.deleted_fs_outputs_flow_steps {
            for original_flow_step in deleted_fs_outputs_flow_steps {
                if let Some(removed_output_ids_by_node) = self
                    .removed_output_ids_by_flow_step
                    .as_ref()
                    .map_or(None, |removed_output_ids_by_flow_step| {
                        removed_output_ids_by_flow_step.get(&original_flow_step.id)
                    })
                {
                    original_flow_step.append_outputs(&removed_output_ids_by_node);

                    original_flow_step.set_merge_context();
                    original_flow_step
                        .update_cb(data)
                        .execute(data.db_session())
                        .await
                        .context("Error undoing delete outputs")?;
                }
            }
        }

        Ok(())
    }

    pub async fn update_description(&mut self, data: &RequestData, branch: &mut Branch) -> Result<(), NodecosmosError> {
        if let Some(edited_flow_step_descriptions) = self.edited_flow_step_descriptions.as_mut() {
            for edited_flow_step_description in edited_flow_step_descriptions {
                let object_id = edited_flow_step_description.object_id;
                let mut text_change = TextChange::new();

                if let Some(original) = self
                    .original_flow_step_descriptions
                    .as_mut()
                    .map_or(None, |original| original.get_mut(&object_id))
                {
                    // skip if description is the same
                    if original.html == edited_flow_step_description.html {
                        continue;
                    }

                    // assign old before updating the flow_step
                    text_change.assign_old(original.markdown.clone());
                }

                edited_flow_step_description.set_original_id();

                // description merge is handled within before_insert callback
                edited_flow_step_description
                    .insert_cb(data)
                    .execute(data.db_session())
                    .await
                    .context("Failed to update flow_step description")?;

                // assign new after updating the flow_step
                text_change.assign_new(edited_flow_step_description.markdown.clone());

                branch
                    .description_change_by_object
                    .get_or_insert_with(HashMap::default)
                    .insert(object_id, text_change);
            }
        }

        Ok(())
    }

    pub async fn undo_update_description(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        if let Some(original_flow_step_descriptions) = &mut self.original_flow_step_descriptions {
            for original_flow_step_description in original_flow_step_descriptions.values_mut() {
                // description merge is handled within before_insert callback, so we use update to revert as
                // it will not trigger merge logic
                original_flow_step_description
                    .update_cb(data)
                    .execute(data.db_session())
                    .await
                    .context("Error undoing update flow step description")?;
            }
        }

        Ok(())
    }
}
