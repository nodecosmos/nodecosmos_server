use charybdis::types::Uuid;
use log::error;

use crate::api::data::RequestData;
use crate::api::types::{ActionObject, ActionTypes};
use crate::errors::NodecosmosError;
use crate::models::comment::Comment;
use crate::models::traits::SanitizeDescription;
use crate::models::udts::Profile;
use crate::resources::sse_broadcast::ModelEvent;

impl Comment {
    pub async fn set_default_values(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
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

    pub async fn emmit_create_event(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        let thread = self.thread(&data.db_session()).await?;

        match thread {
            Some(thread) => {
                let root_id = thread.root_id;

                let res = ModelEvent::new(root_id, ActionTypes::Create(ActionObject::Comment), self)
                    .send(data)
                    .await;

                if let Err(e) = res {
                    error!("Error sending message to room {}: {}", root_id, e);
                }
            }
            None => {
                error!("[after_insert] Thread not initialized");
            }
        }

        Ok(())
    }
}
