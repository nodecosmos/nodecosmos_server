use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::branch::Branch;
use crate::models::description::{find_description, Description, ObjectType};
use crate::models::flow::{find_update_title_flow, Flow, UpdateTitleFlow};
use crate::models::traits::context::ModelContext;
use crate::models::traits::ref_cloned::RefCloned;
use crate::models::traits::{Branchable, FindForBranchMerge, GroupById, GroupByObjId, Pluck};
use crate::models::udts::TextChange;
use charybdis::operations::{DeleteWithCallbacks, Insert, InsertWithCallbacks, UpdateWithCallbacks};
use charybdis::types::Uuid;
use log::warn;
use scylla::CachingSession;
use std::collections::HashMap;

pub struct MergeFlows<'a> {
    branch: &'a Branch,
    restored_flows: Option<Vec<Flow>>,
    created_flows: Option<Vec<Flow>>,
    edited_title_flows: Option<Vec<UpdateTitleFlow>>,
    edited_flow_descriptions: Option<Vec<Description>>,
    original_title_flows: Option<HashMap<Uuid, UpdateTitleFlow>>,
    original_flow_descriptions: Option<HashMap<Uuid, Description>>,
    title_change_by_object: Option<HashMap<Uuid, TextChange>>,
    description_change_by_object: Option<HashMap<Uuid, TextChange>>,
}

impl<'a> MergeFlows<'a> {
    async fn original_title_flows(
        db_session: &CachingSession,
        branch: &Branch,
    ) -> Result<Option<HashMap<Uuid, UpdateTitleFlow>>, NodecosmosError> {
        let node_id = branch.node(&db_session).await?.id;

        if let (Some(ids), Some(edited_wf_node_ids)) = (&branch.edited_title_flows, &branch.edited_workflow_nodes) {
            let ios_by_id = find_update_title_flow!(
                "node_id in ? AND branch_id = ? AND id IN = ?",
                (edited_wf_node_ids, node_id, ids)
            )
            .execute(db_session)
            .await?
            .group_by_id()
            .await?;

            return Ok(Some(ios_by_id));
        }

        Ok(None)
    }

    async fn original_flow_descriptions(
        db_session: &CachingSession,
        branch: &Branch,
    ) -> Result<Option<HashMap<Uuid, Description>>, NodecosmosError> {
        if let Some(ids) = &branch.edited_description_flows {
            let ios_by_id = find_description!("object_id IN ? AND branch_id IN ?", (ids, ids))
                .execute(db_session)
                .await?
                .group_by_obj_id()
                .await?;

            return Ok(Some(ios_by_id));
        }

        Ok(None)
    }

    pub async fn restored_flows(
        branch: &Branch,
        db_session: &CachingSession,
    ) -> Result<Option<Vec<Flow>>, NodecosmosError> {
        if let (Some(edited_workflow_node_ids), Some(restored_flow_ids)) =
            (&branch.edited_workflow_nodes, &branch.restored_flows)
        {
            let mut flows = Flow::find_by_node_ids_and_branch_id_and_ids(
                db_session,
                edited_workflow_node_ids,
                branch.id,
                restored_flow_ids,
            )
            .await?;

            let already_restored_ids = Flow::find_by_node_ids_and_branch_id_and_ids(
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

    pub async fn created_flows(
        branch: &Branch,
        db_session: &CachingSession,
    ) -> Result<Option<Vec<Flow>>, NodecosmosError> {
        if let (Some(edited_workflow_node_ids), Some(created_flow_ids)) =
            (&branch.edited_workflow_nodes, &branch.created_flows)
        {
            let flows = Flow::find_by_node_ids_and_branch_id_and_ids(
                db_session,
                edited_workflow_node_ids,
                branch.id,
                created_flow_ids,
            )
            .await?;

            return Ok(Some(flows));
        }

        Ok(None)
    }

    pub async fn edited_title_flows(
        branch: &Branch,
        db_session: &CachingSession,
    ) -> Result<Option<Vec<UpdateTitleFlow>>, NodecosmosError> {
        if let (Some(edited_workflow_node_ids), Some(edited_title_flows)) =
            (&branch.edited_workflow_nodes, &branch.edited_title_flows)
        {
            let flows = UpdateTitleFlow::find_by_node_ids_and_branch_id_and_ids(
                db_session,
                edited_workflow_node_ids,
                branch.id,
                edited_title_flows,
            )
            .await?;

            let flows = branch.map_original_records(flows, ObjectType::Flow).collect();

            return Ok(Some(flows));
        }

        Ok(None)
    }

    pub async fn edited_flow_descriptions(
        branch: &Branch,
        db_session: &CachingSession,
    ) -> Result<Option<Vec<Description>>, NodecosmosError> {
        if let Some(edited_description_flow_ids) = &branch.edited_description_flows {
            let descriptions = find_description!(
                "branch_id = ? AND object_id IN ?",
                (branch.id, edited_description_flow_ids)
            )
            .execute(db_session)
            .await?
            .try_collect()
            .await?;

            return Ok(Some(
                branch.map_original_objects(ObjectType::Flow, descriptions).collect(),
            ));
        }

        Ok(None)
    }

    pub async fn new(branch: &'a Branch, data: &RequestData) -> Result<Self, NodecosmosError> {
        let restored_flows = Self::restored_flows(&branch, data.db_session()).await?;
        let created_flows = Self::created_flows(&branch, data.db_session()).await?;
        let edited_title_flows = Self::edited_title_flows(&branch, data.db_session()).await?;
        let edited_flow_descriptions = Self::edited_flow_descriptions(&branch, data.db_session()).await?;
        let original_title_flows = Self::original_title_flows(data.db_session(), &branch).await?;
        let original_flow_descriptions = Self::original_flow_descriptions(data.db_session(), &branch).await?;

        Ok(Self {
            branch,
            restored_flows,
            created_flows,
            edited_title_flows,
            edited_flow_descriptions,
            original_title_flows,
            original_flow_descriptions,
            title_change_by_object: None,
            description_change_by_object: None,
        })
    }

    async fn insert_flows(data: &RequestData, merge_flows: &mut Option<Vec<Flow>>) -> Result<(), NodecosmosError> {
        if let Some(merge_flows) = merge_flows {
            for merge_flow in merge_flows {
                merge_flow.set_merge_context();
                merge_flow.set_original_id();
                merge_flow.insert().execute(data.db_session()).await?;
            }
        }

        Ok(())
    }

    pub async fn delete_inserted_flows(
        data: &RequestData,
        merge_flows: &mut Option<Vec<Flow>>,
    ) -> Result<(), NodecosmosError> {
        if let Some(merge_flows) = merge_flows {
            for merge_flow in merge_flows {
                merge_flow.set_merge_context();
                merge_flow.set_original_id();
                merge_flow.delete_cb(data).execute(data.db_session()).await?;
            }
        }

        Ok(())
    }

    pub async fn restore_flows(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        Self::insert_flows(data, &mut self.restored_flows).await?;

        Ok(())
    }

    pub async fn undo_restore_flows(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        Self::delete_inserted_flows(data, &mut self.restored_flows).await?;

        Ok(())
    }

    pub async fn create_flows(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        Self::insert_flows(data, &mut self.created_flows).await?;

        Ok(())
    }

    pub async fn undo_create_flows(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        Self::delete_inserted_flows(data, &mut self.created_flows).await?;

        Ok(())
    }

    pub async fn delete_flows(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        let deleted_flow_ids = self.branch.deleted_flows.ref_cloned();
        let node_id = self.branch.node(data.db_session()).await?.id;

        for deleted_flow_id in deleted_flow_ids {
            let deleted_flow =
                Flow::maybe_find_first_by_node_id_and_branch_id_and_id(node_id, node_id, deleted_flow_id)
                    .execute(data.db_session())
                    .await?;

            if let Some(mut deleted_flow) = deleted_flow {
                deleted_flow.set_original_id();
                deleted_flow.set_merge_context();
                deleted_flow.delete_cb(data).execute(data.db_session()).await?;
            } else {
                warn!(
                    "Failed to find deleted io with id {} and branch id {}",
                    deleted_flow_id, deleted_flow_id
                );
            }
        }

        Ok(())
    }

    pub async fn undo_delete_flows(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        let deleted_flow_ids = self.branch.deleted_flows.ref_cloned();
        let node_id = self.branch.node(data.db_session()).await?.id;

        for deleted_flow_id in deleted_flow_ids.clone() {
            let deleted_flow =
                Flow::maybe_find_first_by_node_id_and_branch_id_and_id(node_id, self.branch.id, deleted_flow_id)
                    .execute(data.db_session())
                    .await?;

            if let Some(mut deleted_flow) = deleted_flow {
                deleted_flow.set_original_id();
                deleted_flow.insert_cb(data).execute(data.db_session()).await?;
            } else {
                warn!(
                    "Failed to find deleted io with id {} and branch id {}",
                    deleted_flow_id, deleted_flow_id
                );
            }
        }

        Ok(())
    }

    pub async fn update_flows_titles(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        let original_title_flows = match self.original_title_flows.as_ref() {
            Some(ios) => ios,
            None => return Ok(()),
        };

        let edited_flow_titles = match self.edited_title_flows.as_mut() {
            Some(titles) => titles,
            None => return Ok(()),
        };

        for edited_flow_title in edited_flow_titles {
            if let Some(original_flow) = original_title_flows.get(&edited_flow_title.id) {
                if original_flow.title != edited_flow_title.title {
                    let mut text_change = TextChange::new();
                    text_change.assign_old(Some(original_flow.title.clone()));
                    text_change.assign_new(Some(edited_flow_title.title.clone()));

                    edited_flow_title.set_original_id();
                    edited_flow_title.set_merge_context();
                    edited_flow_title.update_cb(data).execute(data.db_session()).await?;

                    self.title_change_by_object
                        .get_or_insert_with(HashMap::default)
                        .insert(edited_flow_title.id, text_change);
                }
            }
        }

        Ok(())
    }

    pub async fn undo_update_flows_titles(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        if let Some(original_title_flows) = &mut self.original_title_flows {
            for original_title_flow in original_title_flows.values_mut() {
                original_title_flow.update_cb(data).execute(data.db_session()).await?;
            }
        }

        Ok(())
    }

    pub async fn update_flows_description(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        let edited_flow_descriptions = match self.edited_flow_descriptions.as_mut() {
            Some(descriptions) => descriptions,
            None => return Ok(()),
        };

        let mut default = HashMap::default();
        let original_flow_descriptions = self.original_flow_descriptions.as_mut().unwrap_or_else(|| &mut default);

        for edited_flow_description in edited_flow_descriptions {
            let object_id = edited_flow_description.object_id;
            let mut default_description = Description {
                object_id,
                branch_id: object_id,
                ..Default::default()
            };

            let original = original_flow_descriptions
                .get_mut(&object_id)
                .unwrap_or_else(|| &mut default_description);

            // init text change for remembrance of diff between old and new description
            let mut text_change = TextChange::new();
            text_change.assign_old(original.markdown.clone());

            // description merge is handled within before_insert callback
            original.base64 = edited_flow_description.base64.clone();
            original.insert_cb(data).execute(data.db_session()).await?;

            // update text change with new description
            text_change.assign_new(original.markdown.clone());
            self.description_change_by_object
                .get_or_insert_with(HashMap::default)
                .insert(object_id, text_change);
        }

        Ok(())
    }

    pub async fn undo_update_flows_description(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        if let Some(original_flow_descriptions) = &mut self.original_flow_descriptions {
            for original_flow_description in original_flow_descriptions.values_mut() {
                // description merge is handled within before_insert callback, so we use update to revert as
                // it will not trigger merge logic
                original_flow_description
                    .update_cb(data)
                    .execute(data.db_session())
                    .await?;
            }
        }

        Ok(())
    }
}
