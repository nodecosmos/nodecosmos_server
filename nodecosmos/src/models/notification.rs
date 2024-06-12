use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::udts::Profile;
use charybdis::batch::ModelBatch;
use charybdis::macros::charybdis_model;
use charybdis::types::{Boolean, Frozen, Text, Timestamp, Uuid};
use futures::TryStreamExt;
use scylla::CachingSession;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Deserialize, strum_macros::Display, strum_macros::EnumString)]
pub enum NotificationType {
    NewContributionRequest,
    MergeContributionRequest,
    NewComment,
}

#[charybdis_model(
    table_name = notifications,
    partition_keys = [user_id],
    clustering_keys = [created_at, id],
    local_secondary_indexes = [seen],
    table_options = r#"
        CLUSTERING ORDER BY (created_at DESC)
    "#
)]
#[derive(Serialize, Deserialize, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Notification {
    pub id: Uuid,
    pub user_id: Uuid,
    pub notification_type: Text,
    pub text: Text,
    pub url: Text,
    pub seen: Boolean,
    pub author: Option<Frozen<Profile>>,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

impl Notification {
    pub fn new(
        notification_type: NotificationType,
        text: String,
        url: String,
        author: Option<Frozen<Profile>>,
    ) -> Notification {
        Notification {
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            notification_type: notification_type.to_string(),
            text,
            url,
            seen: false,
            author,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }

    pub async fn mark_all_as_read(db_session: &CachingSession, user_id: Uuid) -> Result<(), NodecosmosError> {
        let notifications = UpdateSeen::find_by_user_id_and_seen(user_id, false)
            .execute(db_session)
            .await?
            .try_filter_map(|mut notification| async move {
                notification.seen = true;
                notification.updated_at = chrono::Utc::now();
                Ok(Some(notification))
            })
            .try_collect::<Vec<UpdateSeen>>()
            .await?;

        UpdateSeen::batch()
            .chunked_update(db_session, &notifications, crate::constants::BATCH_CHUNK_SIZE)
            .await?;

        Ok(())
    }

    pub async fn create_for_receivers(
        &self,
        data: &RequestData,
        receiver_ids: HashSet<Uuid>,
    ) -> Result<(), NodecosmosError> {
        let notifications = receiver_ids
            .into_iter()
            .filter_map(|receiver_id| {
                return if receiver_id != data.current_user.id {
                    Some(Notification {
                        id: Uuid::new_v4(),
                        user_id: receiver_id,
                        ..self.clone()
                    })
                } else {
                    None
                };
            })
            .collect();

        Notification::batch()
            .chunked_insert(data.db_session(), &notifications, crate::constants::BATCH_CHUNK_SIZE)
            .await?;

        Ok(())
    }
}

partial_notification!(UpdateSeen, user_id, created_at, updated_at, id, seen);
