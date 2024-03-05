use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::comment::{find_first_pk_comment, Comment, PkComment};
use crate::models::contribution_request::ContributionRequest;
use charybdis::callbacks::Callbacks;
use charybdis::macros::charybdis_model;
use charybdis::operations::Delete;
use charybdis::types::{Int, Text, Timestamp, Uuid};
use log::error;
use scylla::CachingSession;
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Deserialize)]
pub enum ObjectType {
    ContributionRequest,
    Topic,
}

impl fmt::Display for ObjectType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ObjectType::ContributionRequest => write!(f, "ContributionRequest"),
            ObjectType::Topic => write!(f, "Topic"),
        }
    }
}

impl ObjectType {
    pub fn from_string(s: &str) -> Self {
        match s {
            "ContributionRequest" => ObjectType::ContributionRequest,
            "Topic" => ObjectType::Topic,
            _ => unimplemented!("Unknown object type"),
        }
    }
}

pub enum CommentObject {
    ContributionRequest(ContributionRequest), // Topic(Topic)
}

pub enum ContributionRequestThreadType {
    MainThread,
    NodeAddition,
    NodeDeletion,
    NodeDescription,
}

pub enum ThreadType {
    Topic,
    ContributionRequest(ContributionRequestThreadType),
}

impl fmt::Display for ThreadType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ThreadType::Topic => write!(f, "Topic"),
            ThreadType::ContributionRequest(ContributionRequestThreadType::MainThread) => {
                write!(f, "ContributionRequest::MainThread")
            }
            ThreadType::ContributionRequest(ContributionRequestThreadType::NodeAddition) => {
                write!(f, "ContributionRequest::NodeAddition")
            }
            ThreadType::ContributionRequest(ContributionRequestThreadType::NodeDeletion) => {
                write!(f, "ContributionRequest::NodeDeletion")
            }
            ThreadType::ContributionRequest(ContributionRequestThreadType::NodeDescription) => {
                write!(f, "ContributionRequest::NodeDescription")
            }
        }
    }
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
pub struct CommentThread {
    #[serde(rename = "objectId")]
    pub object_id: Uuid,

    #[serde(rename = "id", default)]
    pub id: Uuid,

    #[serde(rename = "authorId")]
    pub author_id: Option<Uuid>,

    #[serde(rename = "objectType")]
    pub object_type: Text,

    #[serde(rename = "objectNodeId")]
    pub object_node_id: Option<Uuid>,

    #[serde(rename = "threadType")]
    pub thread_type: Text,

    #[serde(rename = "threadNodeId")]
    pub thread_node_id: Option<Uuid>,

    #[serde(rename = "lineNumber")]
    pub line_number: Option<Int>,

    #[serde(rename = "lineContent")]
    pub line_content: Option<Text>,

    #[serde(rename = "createdAt", default = "chrono::Utc::now")]
    pub created_at: Timestamp,

    #[serde(rename = "updatedAt", default = "chrono::Utc::now")]
    pub updated_at: Timestamp,
}

impl CommentThread {
    pub async fn object(&self, db_session: &CachingSession) -> Result<CommentObject, NodecosmosError> {
        match ObjectType::from_string(&self.object_type) {
            ObjectType::ContributionRequest => {
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

    async fn before_insert(&mut self, _session: &CachingSession, data: &RequestData) -> Result<(), Self::Error> {
        let now = chrono::Utc::now();

        self.author_id = Some(data.current_user_id());
        self.id = Uuid::new_v4();
        self.created_at = now;
        self.updated_at = now;

        Ok(())
    }
}
