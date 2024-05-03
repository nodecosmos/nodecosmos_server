use std::collections::HashMap;

use anyhow::Context;
use charybdis::operations::{DeleteWithCallbacks, InsertWithCallbacks, UpdateWithCallbacks};
use charybdis::types::Uuid;
use scylla::CachingSession;
use serde::{Deserialize, Serialize};

use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::branch::Branch;
use crate::models::io::{find_update_title_io, Io, UpdateTitleIo};
use crate::models::traits::{Branchable, GroupById, Pluck};
use crate::models::traits::{ModelContext, ObjectType};
use crate::models::udts::TextChange;

#[derive(Serialize, Deserialize)]
pub struct MergeIos {
    pub restored_ios: Option<Vec<Io>>,
    pub created_ios: Option<Vec<Io>>,
    deleted_ios: Option<Vec<Io>>,
    edited_title_ios: Option<Vec<UpdateTitleIo>>,
    original_title_ios: Option<HashMap<Uuid, UpdateTitleIo>>,
}

impl MergeIos {
    pub async fn restored_ios(
        db_session: &CachingSession,
        branch: &Branch,
    ) -> Result<Option<Vec<Io>>, NodecosmosError> {
        if let Some(restored_io_ids) = &branch.restored_ios {
            let mut branched_ios =
                Io::find_by_branch_id_and_root_id_and_ids(db_session, branch.id, branch.root_id, &restored_io_ids)
                    .await?;
            let already_restored_ids =
                Io::find_by_branch_id_and_root_id_and_ids(db_session, branch.root_id, branch.root_id, &restored_io_ids)
                    .await?
                    .pluck_id_set();

            branched_ios.retain(|branched_node| !already_restored_ids.contains(&branched_node.id));

            return Ok(Some(branched_ios));
        }

        Ok(None)
    }

    pub async fn created_ios(db_session: &CachingSession, branch: &Branch) -> Result<Option<Vec<Io>>, NodecosmosError> {
        if let Some(created_io_ids) = &branch.created_ios {
            let created_ios =
                Io::find_by_branch_id_and_root_id_and_ids(db_session, branch.id, branch.root_id, created_io_ids)
                    .await?;

            return Ok(Some(created_ios));
        }

        Ok(None)
    }

    pub async fn deleted_ios(db_session: &CachingSession, branch: &Branch) -> Result<Option<Vec<Io>>, NodecosmosError> {
        if let Some(deleted_io_ids) = &branch.deleted_ios {
            let deleted_ios = Io::find_by_branch_id_and_root_id_and_ids(
                db_session,
                branch.original_id(),
                branch.root_id,
                deleted_io_ids,
            )
            .await?;

            return Ok(Some(deleted_ios));
        }

        Ok(None)
    }

    pub async fn edited_title_ios(
        db_session: &CachingSession,
        branch: &Branch,
    ) -> Result<Option<Vec<UpdateTitleIo>>, NodecosmosError> {
        if let Some(edited_title_ios) = &branch.edited_title_ios {
            let ios = UpdateTitleIo::find_by_branch_id_and_ids(db_session, branch.id, edited_title_ios).await?;

            let ios = branch.map_original_records(ios, ObjectType::Io).collect();

            return Ok(Some(ios));
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

    pub async fn new(db_session: &CachingSession, branch: &Branch) -> Result<Self, NodecosmosError> {
        let restored_ios = Self::restored_ios(db_session, branch).await?;
        let created_ios = Self::created_ios(db_session, branch).await?;
        let deleted_ios = Self::deleted_ios(db_session, branch).await?;
        let edited_title_ios = Self::edited_title_ios(db_session, branch).await?;
        let original_title_ios = Self::original_title_ios(db_session, &branch).await?;

        Ok(Self {
            restored_ios,
            created_ios,
            deleted_ios,
            edited_title_ios,
            original_title_ios,
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
        Self::insert_ios(data, &mut self.restored_ios)
            .await
            .context("Failed to restore ios")?;

        Ok(())
    }

    pub async fn undo_restore_ios(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        Self::delete_inserted_ios(data, &mut self.restored_ios)
            .await
            .context("Failed to undo restore ios")?;

        Ok(())
    }

    pub async fn create_ios(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        Self::insert_ios(data, &mut self.created_ios)
            .await
            .context("Failed to create ios")?;

        Ok(())
    }

    pub async fn undo_create_ios(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        Self::delete_inserted_ios(data, &mut self.created_ios)
            .await
            .context("Failed to undo create ios")?;

        Ok(())
    }

    pub async fn delete_ios(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        if let Some(deleted_ios) = &mut self.deleted_ios {
            for deleted_io in deleted_ios {
                deleted_io.set_merge_context();
                deleted_io
                    .delete_cb(data)
                    .execute(data.db_session())
                    .await
                    .context("Failed to delete ios")?;
            }
        }

        Ok(())
    }

    pub async fn undo_delete_ios(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        if let Some(deleted_ios) = &mut self.deleted_ios {
            for deleted_io in deleted_ios {
                deleted_io.set_merge_context();
                deleted_io
                    .insert_cb(data)
                    .execute(data.db_session())
                    .await
                    .context("Failed to undo delete ios")?;
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
                    edited_io_title
                        .update_cb(data)
                        .execute(data.db_session())
                        .await
                        .context("Failed to update io title")?;

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
                original_title_io
                    .update_cb(data)
                    .execute(data.db_session())
                    .await
                    .context("Failed to undo update io title")?;
            }
        }

        Ok(())
    }
}
