use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::contribution_request::ContributionRequest;
use charybdis::callbacks::Callbacks;
use charybdis::macros::charybdis_model;
use charybdis::types::{Int, Text, Uuid};
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

#[charybdis_model(
    table_name = comment_threads,
    partition_keys = [object_id],
    clustering_keys = [id],
)]
#[derive(Serialize, Deserialize, Default, Clone)]
pub struct CommentThread {
    #[serde(rename = "objectId")]
    pub object_id: Uuid,

    pub id: Uuid,

    #[serde(rename = "authorId")]
    pub author_id: Option<Uuid>,

    #[serde(rename = "objectType")]
    pub object_type: Text,

    #[serde(rename = "threadType")]
    pub thread_type: Text,

    #[serde(rename = "nodeId")]
    pub node_id: Option<Uuid>,

    #[serde(rename = "lineNumber")]
    pub line_number: Option<Int>,

    #[serde(rename = "lineContent")]
    pub line_content: Option<Text>,
}

impl CommentThread {
    pub async fn object(&self, db_session: &CachingSession) -> Result<CommentObject, NodecosmosError> {
        match ObjectType::from_string(&self.object_type) {
            ObjectType::ContributionRequest => {
                return match self.node_id {
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
}

impl Callbacks for CommentThread {
    type Extension = RequestData;
    type Error = NodecosmosError;

    async fn before_insert(&mut self, _session: &CachingSession, data: &RequestData) -> Result<(), Self::Error> {
        self.author_id = Some(data.current_user_id());

        Ok(())
    }
}
