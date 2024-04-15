use charybdis::callbacks::Callbacks;
use charybdis::macros::charybdis_model;
use charybdis::types::{Frozen, Text, Timestamp, Uuid};
use log::error;
use scylla::CachingSession;
use serde::{Deserialize, Serialize};

use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::comment_thread::CommentThread;
use crate::models::traits::SanitizeDescription;
use crate::models::udts::Profile;

mod create;

#[charybdis_model(
    table_name = comments,
    partition_keys = [object_id],
    clustering_keys = [thread_id, id],
    local_secondary_indexes = [],
)]
#[derive(Serialize, Deserialize, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Comment {
    #[serde(default)]
    pub object_id: Uuid,

    #[serde(default)]
    pub thread_id: Uuid,

    #[serde(default)]
    pub id: Uuid,

    pub content: Text,
    pub author_id: Option<Uuid>,
    pub author: Option<Frozen<Profile>>,

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

        Ok(())
    }

    async fn after_insert(&mut self, _db_session: &CachingSession, data: &RequestData) -> Result<(), Self::Error> {
        self.emmit_create_event(data).await?;

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
                CommentThread::find_by_object_id_and_id(self.object_id, self.thread_id)
                    .execute(db_session)
                    .await?,
            );
        }

        Ok(self.thread.as_mut())
    }
}

partial_comment!(PkComment, object_id, thread_id, id);

partial_comment!(UpdateContentComment, object_id, thread_id, id, content, updated_at);

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

partial_comment!(DeleteComment, object_id, thread_id, id);

impl Callbacks for DeleteComment {
    type Extension = RequestData;
    type Error = NodecosmosError;

    async fn after_delete(&mut self, _db_session: &CachingSession, data: &RequestData) -> Result<(), NodecosmosError> {
        let self_clone = self.clone();
        let data = data.clone();

        tokio::spawn(async move {
            let thread = CommentThread::find_by_object_id_and_id(self_clone.object_id, self_clone.thread_id)
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
