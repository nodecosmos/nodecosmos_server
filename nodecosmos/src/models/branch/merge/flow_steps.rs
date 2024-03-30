use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::branch::Branch;
use crate::models::description::{find_description, Description, ObjectType};
use crate::models::flow_step::{FlowStep, UpdateInputIdsFlowStep, UpdateNodeIdsFlowStep, UpdateOutputIdsFlowStep};
use crate::models::traits::context::ModelContext;
use crate::models::traits::{Branchable, FindForBranchMerge, GroupById, GroupByObjId, Pluck};
use crate::models::udts::TextChange;
use charybdis::operations::{DeleteWithCallbacks, Insert, InsertWithCallbacks, UpdateWithCallbacks};
use charybdis::types::{Set, Uuid};
use log::warn;
use scylla::CachingSession;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize)]
pub struct MergeFlowSteps {
    restored_flow_steps: Option<Vec<FlowStep>>,
    created_flow_steps: Option<Vec<FlowStep>>,
    edited_flow_step_descriptions: Option<Vec<Description>>,
    created_fs_nodes_flow_steps: Option<Vec<UpdateNodeIdsFlowStep>>,
    created_fs_inputs_flow_steps: Option<Vec<UpdateInputIdsFlowStep>>,
    created_fs_outputs_flow_steps: Option<Vec<UpdateOutputIdsFlowStep>>,
    original_flow_step_descriptions: Option<HashMap<Uuid, Description>>,
    title_change_by_object: Option<HashMap<Uuid, TextChange>>,
    description_change_by_object: Option<HashMap<Uuid, TextChange>>,
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
                .group_by_obj_id()
                .await?;

            return Ok(Some(ios_by_id));
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
            let mut flows = FlowStep::find_by_node_ids_and_branch_id_and_ids(
                db_session,
                edited_workflow_node_ids,
                branch.id,
                restored_flow_step_ids,
            )
            .await?;

            let already_restored_ids = FlowStep::find_by_node_ids_and_branch_id_and_ids(
                db_session,
                edited_workflow_node_ids,
                branch.id,
                edited_workflow_node_ids,
            )
            .await?
            .pluck_id_set();

            flows.retain(|flow| !already_restored_ids.contains(&flow.id));

            return Ok(Some(flows));
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
            let flows = FlowStep::find_by_node_ids_and_branch_id_and_ids(
                db_session,
                edited_workflow_node_ids,
                branch.id,
                created_flow_step_ids,
            )
            .await?;

            return Ok(Some(flows));
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

    pub async fn created_fs_nodes_flow_steps(
        branch: &Branch,
        db_session: &CachingSession,
    ) -> Result<Option<Vec<UpdateNodeIdsFlowStep>>, NodecosmosError> {
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

            let flow_steps = branch.map_original_records(flow_steps, ObjectType::FlowStep).collect();

            return Ok(Some(flow_steps));
        }

        Ok(None)
    }

    pub async fn created_fs_inputs_flow_steps(
        branch: &Branch,
        db_session: &CachingSession,
    ) -> Result<Option<Vec<UpdateInputIdsFlowStep>>, NodecosmosError> {
        if let (Some(edited_workflow_node_ids), Some(created_flow_step_inputs_by_node)) =
            (&branch.edited_workflow_nodes, &branch.created_flow_step_inputs_by_node)
        {
            let flow_step_ids: Set<Uuid> = created_flow_step_inputs_by_node.keys().cloned().collect();
            let flow_steps = UpdateInputIdsFlowStep::find_by_node_ids_and_branch_id_and_ids(
                db_session,
                edited_workflow_node_ids,
                branch.id,
                &flow_step_ids,
            )
            .await?;

            let flow_steps = branch.map_original_records(flow_steps, ObjectType::FlowStep).collect();

            return Ok(Some(flow_steps));
        }

        Ok(None)
    }

    pub async fn created_fs_outputs_flow_steps(
        branch: &Branch,
        db_session: &CachingSession,
    ) -> Result<Option<Vec<UpdateOutputIdsFlowStep>>, NodecosmosError> {
        if let (Some(edited_workflow_node_ids), Some(created_flow_step_outputs_by_node)) =
            (&branch.edited_workflow_nodes, &branch.created_flow_step_outputs_by_node)
        {
            let flow_step_ids: Set<Uuid> = created_flow_step_outputs_by_node.keys().cloned().collect();
            let flow_steps = UpdateOutputIdsFlowStep::find_by_node_ids_and_branch_id_and_ids(
                db_session,
                edited_workflow_node_ids,
                branch.id,
                &flow_step_ids,
            )
            .await?;

            let flow_steps = branch.map_original_records(flow_steps, ObjectType::FlowStep).collect();

            return Ok(Some(flow_steps));
        }

        Ok(None)
    }

    pub async fn new(branch: &Branch, data: &RequestData) -> Result<Self, NodecosmosError> {
        let restored_flow_steps = Self::restored_flow_steps(&branch, data.db_session()).await?;
        let created_flow_steps = Self::created_flow_steps(&branch, data.db_session()).await?;
        let edited_flow_step_descriptions = Self::edited_flow_step_descriptions(&branch, data.db_session()).await?;
        let original_flow_step_descriptions = Self::original_flow_step_descriptions(data.db_session(), &branch).await?;
        let created_fs_nodes_flow_steps = Self::created_fs_nodes_flow_steps(&branch, data.db_session()).await?;
        let created_fs_inputs_flow_steps = Self::created_fs_inputs_flow_steps(&branch, data.db_session()).await?;
        let created_fs_outputs_flow_steps = Self::created_fs_outputs_flow_steps(&branch, data.db_session()).await?;

        Ok(Self {
            restored_flow_steps,
            created_flow_steps,
            edited_flow_step_descriptions,
            original_flow_step_descriptions,
            created_fs_nodes_flow_steps,
            created_fs_inputs_flow_steps,
            created_fs_outputs_flow_steps,
            title_change_by_object: None,
            description_change_by_object: None,
        })
    }

    async fn insert_flow_steps(
        data: &RequestData,
        branch: &Branch,
        merge_flow_steps: &mut Option<Vec<FlowStep>>,
    ) -> Result<(), NodecosmosError> {
        if let Some(merge_flow_steps) = merge_flow_steps {
            for merge_flow_step in merge_flow_steps {
                // For branch-created flow_steps, surrounding flow_steps should be already in sync.
                // We need to init them so flow_step insertion doesn't fail. Otherwise flow step may have
                // next_flow_step_id that is present in branch but it's not created for original flow_step.
                if let Some(prev_flow_step_id) = merge_flow_step.prev_flow_step_id {
                    if branch
                        .created_flow_steps
                        .as_ref()
                        .is_some_and(|cfs| cfs.contains(&prev_flow_step_id))
                    {
                        merge_flow_step.prev_flow_step(data.db_session()).await?;
                    }
                }

                if let Some(next_flow_step_id) = merge_flow_step.next_flow_step_id {
                    if branch
                        .created_flow_steps
                        .as_ref()
                        .is_some_and(|cfs| cfs.contains(&next_flow_step_id))
                    {
                        merge_flow_step.next_flow_step(data.db_session()).await?;
                    }
                }

                merge_flow_step.set_merge_context();
                merge_flow_step.set_original_id();
                merge_flow_step.insert_cb(data).execute(data.db_session()).await?;
            }
        }

        Ok(())
    }

    pub async fn delete_inserted_flow_steps(
        data: &RequestData,
        branch: &Branch,
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
        Self::insert_flow_steps(data, self.branch, &mut self.restored_flow_steps).await?;

        Ok(())
    }

    pub async fn undo_restore_flow_steps(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        Self::delete_inserted_flow_steps(data, self.branch, &mut self.restored_flow_steps).await?;

        Ok(())
    }

    pub async fn create_flow_steps(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        Self::insert_flow_steps(data, self.branch, &mut self.created_flow_steps).await?;

        Ok(())
    }

    pub async fn undo_create_flow_steps(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        Self::delete_inserted_flow_steps(data, self.branch, &mut self.created_flow_steps).await?;

        Ok(())
    }

    pub async fn delete_flow_steps(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        let deleted_flow_step_ids = self.branch.deleted_flow_steps.ref_cloned();
        let node_id = self.branch.node(data.db_session()).await?.id;

        for deleted_flow_step_id in deleted_flow_step_ids {
            let deleted_flow_step =
                FlowStep::maybe_find_first_by_node_id_and_branch_id_and_id(node_id, node_id, deleted_flow_step_id)
                    .execute(data.db_session())
                    .await?;

            if let Some(mut deleted_flow_step) = deleted_flow_step {
                deleted_flow_step.delete_cb(data).execute(data.db_session()).await?;
            } else {
                warn!(
                    "Failed to find deleted io with id {} and branch id {}",
                    deleted_flow_step_id, deleted_flow_step_id
                );
            }
        }

        Ok(())
    }

    pub async fn undo_delete_flow_steps(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        let deleted_flow_step_ids = self.branch.deleted_flow_steps.ref_cloned();
        let node_id = self.branch.node(data.db_session()).await?.id;

        for deleted_flow_step_id in deleted_flow_step_ids.clone() {
            let deleted_flow_step = FlowStep::maybe_find_first_by_node_id_and_branch_id_and_id(
                node_id,
                self.branch.id,
                deleted_flow_step_id,
            )
            .execute(data.db_session())
            .await?;

            if let Some(mut deleted_flow_step) = deleted_flow_step {
                deleted_flow_step.set_original_id();
                deleted_flow_step.insert_cb(data).execute(data.db_session()).await?;
            } else {
                warn!(
                    "Failed to find deleted io with id {} and branch id {}",
                    deleted_flow_step_id, deleted_flow_step_id
                );
            }
        }

        Ok(())
    }

    pub async fn update_flow_steps_description(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        let edited_flow_step_descriptions = match self.edited_flow_step_descriptions.as_mut() {
            Some(descriptions) => descriptions,
            None => return Ok(()),
        };

        let mut default = HashMap::default();
        let original_flow_step_descriptions = self
            .original_flow_step_descriptions
            .as_mut()
            .unwrap_or_else(|| &mut default);

        for edited_flow_step_description in edited_flow_step_descriptions {
            let object_id = edited_flow_step_description.object_id;
            let mut default_description = Description {
                object_id,
                branch_id: object_id,
                ..Default::default()
            };

            let original = original_flow_step_descriptions
                .get_mut(&object_id)
                .unwrap_or_else(|| &mut default_description);

            // init text change for remembrance of diff between old and new description
            let mut text_change = TextChange::new();
            text_change.assign_old(original.markdown.clone());

            // description merge is handled within before_insert callback
            original.base64 = edited_flow_step_description.base64.clone();
            original.insert_cb(data).execute(data.db_session()).await?;

            // update text change with new description
            text_change.assign_new(original.markdown.clone());
            self.description_change_by_object
                .get_or_insert_with(HashMap::default)
                .insert(object_id, text_change);
        }

        Ok(())
    }

    pub async fn undo_update_flow_steps_description(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        if let Some(original_flow_step_descriptions) = &mut self.original_flow_step_descriptions {
            for original_flow_step_description in original_flow_step_descriptions.values_mut() {
                // description merge is handled within before_insert callback, so we use update to revert as
                // it will not trigger merge logic
                original_flow_step_description
                    .update_cb(data)
                    .execute(data.db_session())
                    .await?;
            }
        }

        Ok(())
    }
}
