use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::branch::Branch;
use crate::models::description::{find_description, Description, ObjectType};
use crate::models::input_output::{find_update_title_io, Io, UpdateTitleIo};
use crate::models::traits::ModelContext;
use crate::models::traits::{Branchable, GroupById, GroupByObjectId, Pluck};
use crate::models::udts::TextChange;
use charybdis::operations::{DeleteWithCallbacks, InsertWithCallbacks, UpdateWithCallbacks};
use charybdis::types::Uuid;
use scylla::CachingSession;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize)]
pub struct MergeIos {
    restored_ios: Option<Vec<Io>>,
    pub created_ios: Option<Vec<Io>>,
    deleted_ios: Option<Vec<Io>>,
    edited_title_ios: Option<Vec<UpdateTitleIo>>,
    edited_io_descriptions: Option<Vec<Description>>,
    original_title_ios: Option<HashMap<Uuid, UpdateTitleIo>>,
    original_ios_descriptions: Option<HashMap<Uuid, Description>>,
}

impl MergeIos {
    pub async fn restored_ios(
        branch: &Branch,
        db_session: &CachingSession,
    ) -> Result<Option<Vec<Io>>, NodecosmosError> {
        if let Some(restored_io_ids) = &branch.restored_ios {
            let mut branched_ios =
                Io::find_by_root_id_and_branch_id_and_ids(db_session, branch.root_id, branch.id, &restored_io_ids)
                    .await?;
            let already_restored_ids =
                Io::find_by_root_id_and_branch_id_and_ids(db_session, branch.root_id, branch.root_id, &restored_io_ids)
                    .await?
                    .pluck_id_set();

            branched_ios.retain(|branched_node| !already_restored_ids.contains(&branched_node.id));

            return Ok(Some(branched_ios));
        }

        Ok(None)
    }

    pub async fn created_ios(branch: &Branch, db_session: &CachingSession) -> Result<Option<Vec<Io>>, NodecosmosError> {
        if let Some(created_io_ids) = &branch.created_ios {
            let created_ios =
                Io::find_by_root_id_and_branch_id_and_ids(db_session, branch.root_id, branch.id, created_io_ids)
                    .await?;

            return Ok(Some(created_ios));
        }

        Ok(None)
    }

    pub async fn deleted_ios(branch: &Branch, db_session: &CachingSession) -> Result<Option<Vec<Io>>, NodecosmosError> {
        if let Some(deleted_io_ids) = &branch.deleted_ios {
            let deleted_ios =
                Io::find_by_root_id_and_branch_id_and_ids(db_session, branch.root_id, branch.root_id, deleted_io_ids)
                    .await?;

            return Ok(Some(deleted_ios));
        }

        Ok(None)
    }

    pub async fn edited_title_ios(
        branch: &Branch,
        db_session: &CachingSession,
    ) -> Result<Option<Vec<UpdateTitleIo>>, NodecosmosError> {
        if let Some(edited_title_ios) = &branch.edited_title_ios {
            let ios = UpdateTitleIo::find_by_root_id_and_branch_id_and_ids(
                db_session,
                branch.root_id,
                branch.id,
                edited_title_ios,
            )
            .await?;

            let ios = branch.map_original_records(ios, ObjectType::Io).collect();

            return Ok(Some(ios));
        }

        Ok(None)
    }

    pub async fn edited_io_descriptions(
        branch: &Branch,
        db_session: &CachingSession,
    ) -> Result<Option<Vec<Description>>, NodecosmosError> {
        if let Some(edited_description_ios) = &branch.edited_description_ios {
            let descriptions =
                find_description!("branch_id = ? AND object_id IN ?", (branch.id, edited_description_ios))
                    .execute(db_session)
                    .await?
                    .try_collect()
                    .await?;

            return Ok(Some(
                branch.map_original_objects(ObjectType::Io, descriptions).collect(),
            ));
        }

        Ok(None)
    }

    async fn original_title_ios(
        db_session: &CachingSession,
        branch: &Branch,
    ) -> Result<Option<HashMap<Uuid, UpdateTitleIo>>, NodecosmosError> {
        let root_id = branch.node(&db_session).await?.root_id;

        if let Some(ids) = &branch.edited_title_ios {
            let ios_by_id =
                find_update_title_io!("root_id = ? AND branch_id = ? AND id IN = ?", (root_id, root_id, ids))
                    .execute(db_session)
                    .await?
                    .group_by_id()
                    .await?;

            return Ok(Some(ios_by_id));
        }

        Ok(None)
    }

    async fn original_ios_description(
        db_session: &CachingSession,
        branch: &Branch,
    ) -> Result<Option<HashMap<Uuid, Description>>, NodecosmosError> {
        if let Some(ids) = &branch.edited_description_ios {
            let ios_by_id = find_description!("object_id IN ? AND branch_id IN ?", (ids, ids))
                .execute(db_session)
                .await?
                .group_by_object_id()
                .await?;

            return Ok(Some(ios_by_id));
        }

        Ok(None)
    }

    pub async fn new(branch: &Branch, data: &RequestData) -> Result<Self, NodecosmosError> {
        let restored_ios = Self::restored_ios(branch, data.db_session()).await?;
        let created_ios = Self::created_ios(branch, data.db_session()).await?;
        let deleted_ios = Self::deleted_ios(branch, data.db_session()).await?;
        let edited_title_ios = Self::edited_title_ios(branch, data.db_session()).await?;
        let edited_io_descriptions = Self::edited_io_descriptions(branch, data.db_session()).await?;
        let original_title_ios = Self::original_title_ios(data.db_session(), &branch).await?;
        let original_ios_descriptions = Self::original_ios_description(data.db_session(), &branch).await?;

        Ok(Self {
            restored_ios,
            created_ios,
            deleted_ios,
            edited_title_ios,
            edited_io_descriptions,
            original_title_ios,
            original_ios_descriptions,
        })
    }

    async fn insert_ios(data: &RequestData, merge_ios: &mut Option<Vec<Io>>) -> Result<(), NodecosmosError> {
        if let Some(merge_ios) = merge_ios {
            for merge_io in merge_ios {
                merge_io.set_merge_context();
                merge_io.set_original_id();
                merge_io.insert_cb(data).execute(data.db_session()).await?;
            }
        }

        Ok(())
    }

    pub async fn delete_inserted_ios(
        data: &RequestData,
        merge_ios: &mut Option<Vec<Io>>,
    ) -> Result<(), NodecosmosError> {
        if let Some(merge_ios) = merge_ios {
            for merge_io in merge_ios {
                merge_io.set_merge_context();
                merge_io.set_original_id();
                merge_io.delete_cb(data).execute(data.db_session()).await?;
            }
        }

        Ok(())
    }

    pub async fn restore_ios(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        Self::insert_ios(data, &mut self.restored_ios).await?;

        Ok(())
    }

    pub async fn undo_restore_ios(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        Self::delete_inserted_ios(data, &mut self.restored_ios).await?;

        Ok(())
    }

    pub async fn create_ios(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        Self::insert_ios(data, &mut self.created_ios).await?;

        Ok(())
    }

    pub async fn undo_create_ios(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        Self::delete_inserted_ios(data, &mut self.created_ios).await?;

        Ok(())
    }

    pub async fn delete_ios(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        if let Some(deleted_ios) = &mut self.deleted_ios {
            for deleted_io in deleted_ios {
                deleted_io.set_merge_context();
                deleted_io.delete_cb(data).execute(data.db_session()).await?;
            }
        }

        Ok(())
    }

    pub async fn undo_delete_ios(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        if let Some(deleted_ios) = &mut self.deleted_ios {
            for deleted_io in deleted_ios {
                deleted_io.set_merge_context();
                deleted_io.insert_cb(data).execute(data.db_session()).await?;
            }
        }

        Ok(())
    }

    pub async fn update_title(&mut self, data: &RequestData, branch: &mut Branch) -> Result<(), NodecosmosError> {
        let original_title_ios = match self.original_title_ios.as_ref() {
            Some(ios) => ios,
            None => return Ok(()),
        };

        let edited_io_titles = match self.edited_title_ios.as_mut() {
            Some(titles) => titles,
            None => return Ok(()),
        };

        for edited_io_title in edited_io_titles {
            if let Some(original_io) = original_title_ios.get(&edited_io_title.id) {
                if original_io.title != edited_io_title.title {
                    let mut text_change = TextChange::new();
                    text_change.assign_old(original_io.title.clone());
                    text_change.assign_new(edited_io_title.title.clone());

                    edited_io_title.set_merge_context();
                    edited_io_title.set_original_id();
                    edited_io_title.update_cb(data).execute(data.db_session()).await?;

                    branch
                        .title_change_by_object
                        .get_or_insert_with(HashMap::default)
                        .insert(edited_io_title.id, text_change);
                }
            }
        }

        Ok(())
    }

    pub async fn undo_update_title(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        if let Some(original_title_ios) = &mut self.original_title_ios {
            for original_title_io in original_title_ios.values_mut() {
                original_title_io.update_cb(data).execute(data.db_session()).await?;
            }
        }

        Ok(())
    }

    pub async fn update_description(&mut self, data: &RequestData, branch: &mut Branch) -> Result<(), NodecosmosError> {
        let edited_io_descriptions = match self.edited_io_descriptions.as_mut() {
            Some(descriptions) => descriptions,
            None => return Ok(()),
        };

        let mut default = HashMap::default();
        let original_ios_description = self.original_ios_descriptions.as_mut().unwrap_or_else(|| &mut default);

        for edited_io_description in edited_io_descriptions {
            let object_id = edited_io_description.object_id;
            let mut default_description = Description {
                object_id,
                branch_id: object_id,
                ..Default::default()
            };

            let original = original_ios_description
                .get_mut(&object_id)
                .unwrap_or_else(|| &mut default_description);

            // init text change for remembrance of diff between old and new description
            let mut text_change = TextChange::new();
            text_change.assign_old(original.markdown.clone());

            // description merge is handled within before_insert callback
            original.base64 = edited_io_description.base64.clone();
            original.insert_cb(data).execute(data.db_session()).await?;

            // update text change with new description
            text_change.assign_new(original.markdown.clone());
            branch
                .description_change_by_object
                .get_or_insert_with(HashMap::default)
                .insert(object_id, text_change);
        }

        Ok(())
    }

    pub async fn undo_update_description(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        if let Some(original_ios_description) = &mut self.original_ios_descriptions {
            for original_io_description in original_ios_description.values_mut() {
                // description merge is handled within before_insert callback, so we use update to revert as
                // it will not trigger merge logic
                original_io_description
                    .update_cb(data)
                    .execute(data.db_session())
                    .await?;
            }
        }

        Ok(())
    }
}
