pub mod callbacks;

use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::comment_thread::CommentThread;
use crate::models::contribution_request::ContributionRequest;
use crate::models::node::context::Context;
use crate::models::udts::Profile;
use crate::models::utils::{impl_default_callbacks, updated_at_cb_fn};
use charybdis::callbacks::Callbacks;
use charybdis::macros::charybdis_model;
use charybdis::model::AsNative;
use charybdis::operations::Find;
use charybdis::types::{Frozen, Int, SmallInt, Text, Timestamp, Uuid};
use log::{error, warn};
use scylla::CachingSession;
use serde::{Deserialize, Serialize};
use std::fmt;

#[charybdis_model(
    table_name = comments,
    partition_keys = [object_id],
    clustering_keys = [thread_id, id],
    local_secondary_indexes = [],
)]
#[derive(Serialize, Deserialize, Default, Clone)]
pub struct Comment {
    #[serde(rename = "objectId", default)]
    pub object_id: Uuid,

    #[serde(rename = "threadId", default)]
    pub thread_id: Uuid,

    #[serde(default)]
    pub id: Uuid,

    pub content: Text,

    #[serde(rename = "authorId")]
    pub author_id: Option<Uuid>,

    pub author: Option<Frozen<Profile>>,

    #[serde(rename = "createdAt", default = "chrono::Utc::now")]
    pub created_at: Timestamp,

    #[serde(rename = "updatedAt", default = "chrono::Utc::now")]
    pub updated_at: Timestamp,

    #[charybdis(ignore)]
    #[serde(skip)]
    pub thread: Option<CommentThread>,
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

partial_comment!(DeleteComment, object_id, thread_id, id);
