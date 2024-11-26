use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::branch::Branch;
use crate::models::description::Description;
use crate::models::traits::{Branchable, FindForBranchMerge, GroupByObjectId};
use crate::models::udts::TextChange;
use anyhow::Context;
use charybdis::operations::{DeleteWithCallbacks, InsertWithCallbacks, UpdateWithCallbacks};
use charybdis::types::Uuid;
use scylla::CachingSession;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Serialize, Deserialize, Default)]
pub struct MergeDescriptions {
    pub edited_descriptions: Vec<Description>,
    pub deleted_descriptions: Vec<Description>,
    pub original_descriptions: HashMap<Uuid, Description>,
}

impl MergeDescriptions {
    pub async fn new(db_session: &CachingSession, branch: &Branch) -> Result<Self, NodecosmosError> {
        let mut edited_descriptions = Vec::new();
        let mut deleted_descriptions = Vec::new();
        let mut original_descriptions = HashMap::new();

        let edited_object_ids = branch.all_edited_description_ids();
        let deleted_object_ids = branch.all_deleted_object_ids();

        if !edited_object_ids.is_empty() || !deleted_object_ids.is_empty() {
            if !edited_object_ids.is_empty() {
                edited_descriptions = Description::find_by_branch_id_and_ids(db_session, branch.id, &edited_object_ids)
                    .await?
                    .try_collect()
                    .await?;
            }

            if !deleted_object_ids.is_empty() {
                deleted_descriptions =
                    Description::find_by_branch_id_and_ids(db_session, branch.id, &deleted_object_ids)
                        .await?
                        .try_collect()
                        .await?;
            }

            let combined_ids = edited_object_ids
                .union(&deleted_object_ids)
                .cloned()
                .collect::<HashSet<Uuid>>();

            original_descriptions =
                Description::find_by_branch_id_and_ids(db_session, branch.original_id(), &combined_ids)
                    .await?
                    .group_by_object_id()
                    .await?;
        }

        Ok(Self {
            edited_descriptions,
            deleted_descriptions,
            original_descriptions,
        })
    }

    pub async fn update_descriptions(
        &mut self,
        data: &RequestData,
        branch: &mut Branch,
    ) -> Result<(), NodecosmosError> {
        for edited_description in self.edited_descriptions.iter_mut() {
            let object_id = edited_description.object_id;
            let mut text_change = TextChange::new();

            if let Some(original) = self.original_descriptions.get_mut(&object_id) {
                // skip if description is the same
                if original.html == edited_description.html {
                    continue;
                }

                // assign old before updating the node
                text_change.assign_old(original.markdown.clone());
            }

            edited_description.set_original_id();

            // description merge is handled within before_insert callback
            edited_description
                .insert_cb(data)
                .execute(data.db_session())
                .await
                .context("Failed to update node description")?;

            // assign new after updating the node
            text_change.assign_new(edited_description.markdown.clone());

            branch
                .description_change_by_object
                .get_or_insert_with(HashMap::default)
                .insert(object_id, text_change);
        }

        Ok(())
    }

    pub async fn undo_update_description(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        for edited_description in self.edited_descriptions.iter_mut() {
            // update to avoid merge
            if let Some(original) = self.original_descriptions.get_mut(&edited_description.object_id) {
                original
                    .update_cb(data)
                    .execute(data.db_session())
                    .await
                    .context("Failed to update node description")?;
            }
        }

        Ok(())
    }

    pub async fn delete_descriptions(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        for deleted_description in self.deleted_descriptions.iter_mut() {
            deleted_description
                .delete_cb(data)
                .execute(data.db_session())
                .await
                .context("Failed to update node description")?;
        }

        Ok(())
    }

    pub async fn undo_delete_descriptions(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        for deleted_description in self.deleted_descriptions.iter_mut() {
            // update to avoid merge
            if let Some(description) = self.original_descriptions.get_mut(&deleted_description.object_id) {
                description
                    .update_cb(data)
                    .execute(data.db_session())
                    .await
                    .context("Failed to update node description")?;
            }
        }

        Ok(())
    }
}
