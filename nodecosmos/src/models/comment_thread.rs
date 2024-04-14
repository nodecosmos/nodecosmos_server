use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::comment::{find_first_pk_comment, PkComment};
use crate::models::contribution_request::ContributionRequest;
use crate::models::udts::Profile;
use charybdis::callbacks::Callbacks;
use charybdis::macros::charybdis_model;
use charybdis::operations::Delete;
use charybdis::types::{Int, Text, Timestamp, Uuid};
use log::error;
use scylla::CachingSession;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, strum_macros::Display, strum_macros::EnumString)]
pub enum ThreadObjectType {
    ContributionRequest,
    Topic,
}

#[derive(Default, Deserialize, strum_macros::Display, strum_macros::EnumString)]
pub enum ContributionRequestThreadType {
    #[default]
    MainThread,
    NodeAddition,
    NodeDeletion,
    NodeDescription,
}

#[derive(Deserialize, strum_macros::Display, strum_macros::EnumString)]
pub enum ThreadType {
    Topic,
    ContributionRequest(ContributionRequestThreadType),
}

pub enum CommentObject {
    ContributionRequest(ContributionRequest), // Topic(Topic)
}

/// **objectId** corresponds to the following:
/// * **`ContributionRequest['id']`** for ContributionRequest related comments
/// * **`Topic['id']`**  for Topic related comments
///
/// **thread_node_id** if provided corresponds to Node of the following
/// * `ContributionRequestThreadType::NodeAddition`,
/// * `ContributionRequestThreadType::NodeDeletion`,
/// * `ContributionRequestThreadType::NodeDescription`
///
#[charybdis_model(
    table_name = comment_threads,
    partition_keys = [object_id],
    clustering_keys = [id],
)]
#[derive(Serialize, Deserialize, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CommentThread {
    pub object_id: Uuid,
    #[serde(default)]
    pub id: Uuid,
    pub author_id: Option<Uuid>,
    pub author: Option<Profile>,
    pub object_type: Text,
    pub object_node_id: Option<Uuid>,
    pub thread_type: Text,
    pub thread_node_id: Option<Uuid>,
    pub line_number: Option<Int>,
    pub line_content: Option<Text>,

    #[serde(default = "chrono::Utc::now")]
    pub created_at: Timestamp,

    #[serde(default = "chrono::Utc::now")]
    pub updated_at: Timestamp,
}

impl CommentThread {
    pub async fn object(&self, db_session: &CachingSession) -> Result<CommentObject, NodecosmosError> {
        match ThreadObjectType::from(self.object_type.parse()?) {
            ThreadObjectType::ContributionRequest => {
                return match self.object_node_id {
                    Some(node_id) => {
                        let contribution_request = ContributionRequest::find_by_node_id_and_id(node_id, self.object_id)
                            .execute(db_session)
                            .await?;
                        Ok(CommentObject::ContributionRequest(contribution_request))
                    }
                    None => Err(NodecosmosError::NotFound("[object] node_id is empty".to_string())),
                }
            }
            _ => Err(NodecosmosError::NotFound("Object not found".to_string())),
        }
    }

    pub async fn delete_if_no_comments(&self, db_session: &CachingSession) {
        let comment_res = find_first_pk_comment!("object_id = ? AND thread_id = ?", (&self.object_id, &self.id))
            .execute(db_session)
            .await
            .ok();

        if comment_res.is_none() {
            let res = self.delete().execute(db_session).await;

            if let Err(e) = res {
                error!("Error while deleting thread: {}", e);
            }
        }
    }
}

impl Callbacks for CommentThread {
    type Extension = RequestData;
    type Error = NodecosmosError;

    async fn before_insert(&mut self, _db_session: &CachingSession, data: &RequestData) -> Result<(), Self::Error> {
        let now = chrono::Utc::now();

        self.author_id = Some(data.current_user_id());
        self.author = Some(Profile::init_from_current_user(&data.current_user));

        // here is safe to allow client to provide id as request is authenticated with `object_id`
        // we provide `id` to separate main threads from others
        if self.id.is_nil() {
            self.id = Uuid::new_v4();
        }

        self.created_at = now;
        self.updated_at = now;

        Ok(())
    }
}
