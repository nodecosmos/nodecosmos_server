use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::comment::{Comment, DeleteComment, UpdateContentComment};
use crate::models::comment_thread::CommentThread;
use crate::models::traits::SanitizeDescription;
use charybdis::callbacks::Callbacks;
use log::error;
use scylla::CachingSession;

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
