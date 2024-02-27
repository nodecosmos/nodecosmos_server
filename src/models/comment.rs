use crate::errors::NodecosmosError;
use crate::models::contribution_request::ContributionRequest;
use crate::models::udts::Profile;
use crate::models::utils::impl_default_callbacks;
use charybdis::macros::charybdis_model;
use charybdis::operations::Find;
use charybdis::types::{Frozen, Text, Timestamp, Uuid};
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

pub enum CrCommentType {
    MainThread,
    NodeDescription,
}

pub enum CommentType {
    Topic,
    ContributionRequest(),
}

#[charybdis_model(
    table_name = comments,
    partition_keys = [object_id],
    clustering_keys = [branch_id, thread_id, id],
)]
#[derive(Serialize, Deserialize, Default, Clone)]
pub struct Comment {
    #[serde(rename = "objectId")]
    pub object_id: Uuid,

    #[serde(rename = "branchId")]
    pub branch_id: Uuid,

    #[serde(rename = "treadId", default = "Uuid::new_v4")]
    pub thread_id: Uuid,

    pub id: Uuid,

    #[serde(rename = "nodeId")]
    pub node_id: Option<Uuid>,

    #[serde(rename = "objectType")]
    pub object_type: Text,

    #[serde(rename = "commentType")]
    pub comment_type: Text,

    pub content: Text,

    #[serde(rename = "authorId")]
    pub author_id: Uuid,

    pub author: Option<Frozen<Profile>>,

    #[serde(rename = "createdAt")]
    pub created_at: Option<Timestamp>,

    #[serde(rename = "updatedAt")]
    pub updated_at: Option<Timestamp>,
}

impl Comment {
    pub async fn object(&self, db_session: &CachingSession) -> Result<CommentObject, NodecosmosError> {
        match ObjectType::from_string(&self.object_type) {
            ObjectType::ContributionRequest => {
                let contribution_request = ContributionRequest::find_by_node_id_and_id(self.object_id, self.branch_id)
                    .execute(db_session)
                    .await?;
                Ok(CommentObject::ContributionRequest(contribution_request))
            }
            _ => Err(NodecosmosError::NotFound("Object not found".to_string())),
        }
    }
}

partial_comment!(PkComment, object_id, branch_id, thread_id, id);

impl_default_callbacks!(Comment);
