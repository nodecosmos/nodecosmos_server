use std::collections::HashMap;

use anyhow::Context;
use charybdis::operations::{Delete, DeleteWithCallbacks, Insert, InsertWithCallbacks, UpdateWithCallbacks};
use charybdis::types::Uuid;
use scylla::CachingSession;
use serde::{Deserialize, Serialize};

use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::branch::Branch;
use crate::models::flow::{Flow, UpdateTitleFlow};
use crate::models::traits::{Branchable, FindForBranchMerge, GroupById, ObjectType};
use crate::models::traits::{ModelContext, PluckFromStream};
use crate::models::udts::TextChange;

#[derive(Serialize, Deserialize)]
pub struct MergeFlows {
    restored_flows: Option<Vec<Flow>>,
    created_flows: Option<Vec<Flow>>,
    deleted_flows: Option<Vec<Flow>>,
    edited_title_flows: Option<Vec<UpdateTitleFlow>>,
    original_title_flows: Option<HashMap<Uuid, UpdateTitleFlow>>,
}

impl MergeFlows {
    async fn original_title_flows(
        db_session: &CachingSession,
        branch: &Branch,
    ) -> Result<Option<HashMap<Uuid, UpdateTitleFlow>>, NodecosmosError> {
        if let Some(ids) = &branch.edited_title_flows {
            let ios_by_id = UpdateTitleFlow::find_by_branch_id_and_ids(db_session, branch.id, ids)
                .await
                .group_by_id()
                .await?;

            return Ok(Some(ios_by_id));
        }

        Ok(None)
    }

    pub async fn restored_flows(
        db_session: &CachingSession,
        branch: &Branch,
    ) -> Result<Option<Vec<Flow>>, NodecosmosError> {
        if let Some(restored_flow_ids) = &branch.restored_flows {
            let f_stream = Flow::find_by_branch_id_and_ids(db_session, branch.id, restored_flow_ids).await;
            let already_restored_ids =
                Flow::find_by_branch_id_and_ids(db_session, branch.original_id(), restored_flow_ids)
                    .await
                    .pluck_id_set()
                    .await?;
            let mut flows = branch.filter_out_flows_with_deleted_parents(f_stream).await?;

            flows.retain(|flow| !already_restored_ids.contains(&flow.id));

            return Ok(Some(flows));
        }

        Ok(None)
    }

    pub async fn created_flows(
        db_session: &CachingSession,
        branch: &Branch,
    ) -> Result<Option<Vec<Flow>>, NodecosmosError> {
        if let Some(created_flow_ids) = &branch.created_flows {
            let f_stream = Flow::find_by_branch_id_and_ids(db_session, branch.id, created_flow_ids).await;
            let flows = branch.filter_out_flows_with_deleted_parents(f_stream).await?;

            return Ok(Some(flows));
        }

        Ok(None)
    }

    pub async fn deleted_flows(
        db_session: &CachingSession,
        branch: &Branch,
    ) -> Result<Option<Vec<Flow>>, NodecosmosError> {
        if let Some(deleted_flow_ids) = &branch.deleted_flows {
            let f_stream = Flow::find_by_branch_id_and_ids(db_session, branch.original_id(), deleted_flow_ids).await;
            let flows = branch.filter_out_flows_with_deleted_parents(f_stream).await?;

            return Ok(Some(flows));
        }

        Ok(None)
    }

    pub async fn edited_title_flows(
        db_session: &CachingSession,
        branch: &Branch,
    ) -> Result<Option<Vec<UpdateTitleFlow>>, NodecosmosError> {
        if let Some(edited_title_flows) = &branch.edited_title_flows {
            let flows = UpdateTitleFlow::find_by_branch_id_and_ids(db_session, branch.id, edited_title_flows)
                .await
                .try_collect()
                .await?;

            let flows = branch.map_original_records(flows, ObjectType::Flow).collect();

            return Ok(Some(flows));
        }

        Ok(None)
    }

    pub async fn new(db_session: &CachingSession, branch: &Branch) -> Result<Self, NodecosmosError> {
        let restored_flows = Self::restored_flows(db_session, branch).await?;
        let created_flows = Self::created_flows(db_session, branch).await?;
        let deleted_flows = Self::deleted_flows(db_session, branch).await?;
        let edited_title_flows = Self::edited_title_flows(db_session, branch).await?;
        let original_title_flows = Self::original_title_flows(db_session, branch).await?;

        Ok(Self {
            restored_flows,
            created_flows,
            deleted_flows,
            edited_title_flows,
            original_title_flows,
        })
    }

    async fn insert_flows(data: &RequestData, merge_flows: &mut Option<Vec<Flow>>) -> Result<(), NodecosmosError> {
        if let Some(merge_flows) = merge_flows {
            for merge_flow in merge_flows {
                let branch_id = merge_flow.branch_id;
                let current_vertical_index = merge_flow.vertical_index;

                merge_flow.set_merge_context();
                merge_flow.set_original_id();
                merge_flow.insert_cb(data).execute(data.db_session()).await?;

                let new_vertical_index = merge_flow.vertical_index;

                // update vertical index on branch.
                if current_vertical_index != new_vertical_index {
                    // as we can not update clustering keys,
                    // we need to delete and insert again with new vertical index
                    merge_flow.branch_id = branch_id;

                    // delete branched with old vertical index
                    merge_flow.vertical_index = current_vertical_index;
                    merge_flow.delete().execute(data.db_session()).await?;

                    // insert branched with new vertical index
                    merge_flow.vertical_index = new_vertical_index;
                    merge_flow.insert().execute(data.db_session()).await?;

                    // restore original id in case of undo
                    merge_flow.set_original_id();
                }
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
        Self::insert_flows(data, &mut self.restored_flows)
            .await
            .context("Failed to restore flows")?;

        Ok(())
    }

    pub async fn undo_restore_flows(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        Self::delete_inserted_flows(data, &mut self.restored_flows)
            .await
            .context("Failed to undo restore flows")?;

        Ok(())
    }

    pub async fn create_flows(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        Self::insert_flows(data, &mut self.created_flows)
            .await
            .context("Failed to create flows")?;

        Ok(())
    }

    pub async fn undo_create_flows(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        Self::delete_inserted_flows(data, &mut self.created_flows)
            .await
            .context("Failed to undo create flows")?;

        Ok(())
    }

    pub async fn delete_flows(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        if let Some(deleted_flows) = &mut self.deleted_flows {
            for deleted_flow in deleted_flows {
                deleted_flow.set_merge_context();
                deleted_flow
                    .delete_cb(data)
                    .execute(data.db_session())
                    .await
                    .context("Failed to delete flows")?;
            }
        }

        Ok(())
    }

    pub async fn undo_delete_flows(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        if let Some(deleted_flows) = &mut self.deleted_flows {
            for deleted_flow in deleted_flows {
                deleted_flow.set_merge_context();
                deleted_flow
                    .insert_cb(data)
                    .execute(data.db_session())
                    .await
                    .context("Failed to undo delete flows")?;
            }
        }

        Ok(())
    }

    pub async fn update_title(&mut self, data: &RequestData, branch: &mut Branch) -> Result<(), NodecosmosError> {
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
                    edited_flow_title
                        .update_cb(data)
                        .execute(data.db_session())
                        .await
                        .context("Failed to update flow title")?;

                    branch
                        .title_change_by_object
                        .get_or_insert_with(HashMap::default)
                        .insert(edited_flow_title.id, text_change);
                }
            }
        }

        Ok(())
    }

    pub async fn undo_update_title(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        if let Some(original_title_flows) = &mut self.original_title_flows {
            for original_title_flow in original_title_flows.values_mut() {
                original_title_flow
                    .update_cb(data)
                    .execute(data.db_session())
                    .await
                    .context("Failed to undo update flow title")?;
            }
        }

        Ok(())
    }
}
