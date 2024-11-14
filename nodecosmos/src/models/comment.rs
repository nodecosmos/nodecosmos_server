use charybdis::callbacks::Callbacks;
use charybdis::macros::charybdis_model;
use charybdis::types::{Frozen, Text, Timestamp, Uuid};
use log::error;
use scylla::CachingSession;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::comment_thread::{CommentThread, ThreadLocation};
use crate::models::notification::{Notification, NotificationType};
use crate::models::traits::SanitizeDescription;
use crate::models::udts::Profile;

mod create;

#[charybdis_model(
    table_name = comments,
    partition_keys = [branch_id],
    clustering_keys = [thread_id, object_id, id],
    local_secondary_indexes = [],
)]
#[derive(Serialize, Deserialize, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Comment {
    #[serde(default)]
    pub branch_id: Uuid,

    #[serde(default)]
    pub object_id: Uuid,

    #[serde(default)]
    pub thread_id: Uuid,

    #[serde(default)]
    pub id: Uuid,

    pub content: Text,

    #[serde(default)]
    pub author_id: Uuid,
    pub author: Option<Frozen<Profile>>,
    pub url: Text,

    #[serde(default = "chrono::Utc::now")]
    pub created_at: Timestamp,

    #[serde(default = "chrono::Utc::now")]
    pub updated_at: Timestamp,

    #[charybdis(ignore)]
    #[serde(skip)]
    pub thread: Option<CommentThread>,
}

impl Callbacks for Comment {
    type Extension = RequestData;
    type Error = NodecosmosError;

    async fn before_insert(&mut self, _db_session: &CachingSession, data: &RequestData) -> Result<(), NodecosmosError> {
        self.set_default_values(data).await?;
        self.validate_author(data).await?;
        self.validate_url(data).await?;

        self.content.sanitize()?;

        Ok(())
    }

    async fn after_insert(&mut self, _: &CachingSession, data: &RequestData) -> Result<(), Self::Error> {
        let mut self_clone = self.clone();
        let data = data.clone();

        tokio::spawn(async move {
            let _ = self_clone
                .emmit_create_event(&data)
                .await
                .map_err(|e| error!("Error while emitting create event: {}", e));
            let author_id = self_clone.author_id;
            let thread_res = self_clone.thread(data.db_session()).await;
            match thread_res {
                Ok(Some(thread)) => {
                    let _ = thread
                        .push_participant_ids(vec![author_id])
                        .execute(data.db_session())
                        .await
                        .map_err(|e| {
                            error!("Error while updating thread participants: {}", e);
                        });

                    match thread.thread_location() {
                        Ok(ThreadLocation::ContributionRequest(thread_location)) => {
                            let notification_text = thread_location.notification_text().to_string();
                            let mut receiver_ids = thread.participant_ids.get_or_insert_with(HashSet::new).clone();
                            if let Ok(branch) = thread.branch(data.db_session()).await {
                                receiver_ids.insert(branch.owner_id);
                                if let Some(editor_ids) = &branch.editor_ids {
                                    receiver_ids.extend(editor_ids);
                                }
                            }
                            let _ = Notification::new(
                                NotificationType::NewComment,
                                notification_text,
                                self_clone.url,
                                self_clone.author,
                            )
                            .create_for_receivers(&data, receiver_ids)
                            .await
                            .map_err(|e| {
                                error!("Error while creating notification: {}", e);
                            });
                        }
                        Err(e) => error!(
                            "Error getting thread_location while updating thread participants: {}",
                            e
                        ),
                        Ok(ThreadLocation::Thread) => {
                            let notification_text = format!("commented thread: {}", thread.title);
                            let mut receiver_ids = thread.participant_ids.get_or_insert_with(HashSet::new).clone();
                            if let Ok(node) = thread.node(data.db_session()).await {
                                receiver_ids.insert(node.owner_id);
                                receiver_ids.extend(node.editor_ids.clone().unwrap_or_default());
                            }

                            let _ = Notification::new(
                                NotificationType::NewComment,
                                notification_text,
                                self_clone.url,
                                self_clone.author,
                            )
                            .create_for_receivers(&data, receiver_ids)
                            .await
                            .map_err(|e| {
                                error!("Error while creating notification: {}", e);
                            });
                        }
                    }
                }
                Err(e) => error!("Error while updating thread participants: {}", e),
                _ => {}
            }
        });

        Ok(())
    }
}

impl Comment {
    pub fn assign_thread(&mut self, thread: CommentThread) {
        self.thread = Some(thread);
    }

    pub async fn thread(&mut self, db_session: &CachingSession) -> Result<Option<&mut CommentThread>, NodecosmosError> {
        if self.thread.is_none() {
            self.thread = Some(
                CommentThread::find_by_branch_id_and_object_id_and_id(self.branch_id, self.object_id, self.thread_id)
                    .execute(db_session)
                    .await?,
            );
        }

        Ok(self.thread.as_mut())
    }
}

partial_comment!(
    BaseComment,
    branch_id,
    thread_id,
    object_id,
    id,
    content,
    author_id,
    author,
    created_at,
    updated_at
);

partial_comment!(PkComment, branch_id, thread_id, object_id, id);

partial_comment!(
    UpdateContentComment,
    branch_id,
    thread_id,
    object_id,
    id,
    content,
    updated_at
);

impl Callbacks for UpdateContentComment {
    type Extension = Option<()>;
    type Error = NodecosmosError;

    async fn before_update(
        &mut self,
        _db_session: &CachingSession,
        _ext: &Self::Extension,
    ) -> Result<(), NodecosmosError> {
        self.updated_at = chrono::Utc::now();

        self.content.sanitize()?;

        Ok(())
    }
}

partial_comment!(DeleteComment, branch_id, thread_id, object_id, id);

impl Callbacks for DeleteComment {
    type Extension = RequestData;
    type Error = NodecosmosError;

    async fn after_delete(&mut self, _db_session: &CachingSession, data: &RequestData) -> Result<(), NodecosmosError> {
        let self_clone = self.clone();
        let data = data.clone();

        tokio::spawn(async move {
            let thread = CommentThread::find_by_branch_id_and_object_id_and_id(
                self_clone.branch_id,
                self_clone.object_id,
                self_clone.thread_id,
            )
            .execute(data.db_session())
            .await;

            match thread {
                Ok(thread) => {
                    thread.delete_if_no_comments(data.db_session()).await;
                }
                Err(e) => error!("Error while deleting comment: {}", e),
            }
        });

        Ok(())
    }
}
