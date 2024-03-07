use crate::api::data::RequestData;
use crate::api::types::{ActionObject, ActionTypes};
use crate::clients::sse_broadcast::ModelEvent;
use crate::errors::NodecosmosError;
use crate::models::comment::{Comment, DeleteComment, UpdateContentComment};
use crate::models::comment_thread::CommentThread;
use crate::models::traits::SanitizeDescription;
use crate::models::udts::Profile;
use charybdis::callbacks::Callbacks;
use charybdis::types::Uuid;
use log::error;
use scylla::CachingSession;

impl Callbacks for Comment {
    type Extension = RequestData;
    type Error = NodecosmosError;

    async fn before_insert(&mut self, _session: &CachingSession, data: &RequestData) -> Result<(), NodecosmosError> {
        let now = chrono::Utc::now();
        let thread = self.thread(&data.db_session()).await?;

        match thread {
            Some(thread) => {
                let object_id = thread.object_id;
                let thread_id = thread.id;

                self.object_id = object_id;
                self.thread_id = thread_id;
            }
            None => {
                error!("[before_insert] Thread not initialized");
                return Err(NodecosmosError::NotFound("Thread not initialized".to_string()));
            }
        }

        self.id = Uuid::new_v4();
        self.author_id = Some(data.current_user_id());
        self.author = Some(Profile::init_from_current_user(&data.current_user));
        self.content.sanitize()?;
        self.created_at = now;
        self.updated_at = now;

        Ok(())
    }

    async fn after_insert(&mut self, _session: &CachingSession, data: &RequestData) -> Result<(), Self::Error> {
        let thread = self.thread(&data.db_session()).await?;

        match thread {
            Some(thread) => {
                let node_id = &thread.object_node_id.map_or(thread.object_id, |id| id);

                let res = ModelEvent::new(&node_id, ActionTypes::Create(ActionObject::Comment), self)
                    .send(data)
                    .await;

                if let Err(e) = res {
                    error!("Error sending message to room {}: {}", node_id, e);
                }
            }
            None => {
                error!("[after_insert] Thread not initialized");
            }
        }

        Ok(())
    }
}

impl Callbacks for UpdateContentComment {
    type Extension = Option<()>;
    type Error = NodecosmosError;

    async fn before_update(
        &mut self,
        _session: &CachingSession,
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

    async fn after_delete(&mut self, _session: &CachingSession, req_data: &RequestData) -> Result<(), NodecosmosError> {
        let self_clone = self.clone();
        let req_data = req_data.clone();

        tokio::spawn(async move {
            let thread = CommentThread::find_by_object_id_and_id(self_clone.object_id, self_clone.thread_id)
                .execute(req_data.db_session())
                .await;

            match thread {
                Ok(thread) => {
                    thread.delete_if_no_comments(req_data.db_session()).await;
                }
                Err(e) => error!("Error while deleting comment: {}", e),
            }
        });

        Ok(())
    }
}
