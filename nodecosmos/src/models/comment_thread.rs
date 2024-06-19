use charybdis::callbacks::Callbacks;
use charybdis::macros::charybdis_model;
use charybdis::operations::Delete;
use charybdis::types::{Int, Set, Text, Timestamp, Uuid};
use log::error;
use scylla::CachingSession;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::branch::Branch;
use crate::models::comment::{Comment, PkComment};
use crate::models::node::Node;
use crate::models::node_counter::NodeCounter;
use crate::models::udts::Profile;

#[derive(Deserialize, strum_macros::Display, strum_macros::EnumString)]
pub enum ThreadObjectType {
    ContributionRequest,
    Thread,
}

#[derive(Default, Deserialize, strum_macros::Display, strum_macros::EnumString)]
pub enum ContributionRequestThreadLocation {
    #[default]
    MainThread,
    ObjectDescription,
    Node,
    Flow,
    FlowStep,
    InputOutput,
}

#[derive(Deserialize)]
pub enum ThreadLocation {
    Thread,
    ContributionRequest(ContributionRequestThreadLocation),
}

impl FromStr for ThreadLocation {
    type Err = NodecosmosError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Thread" => Ok(ThreadLocation::Thread),
            _ => {
                let parts: Vec<&str> = s.split("::").collect();
                if parts.len() == 2 && parts[0] == "ContributionRequest" {
                    let cr_type = parts[1].parse::<ContributionRequestThreadLocation>().map_err(|_| {
                        NodecosmosError::InternalServerError("Invalid ContributionRequestThreadLocation".to_string())
                    })?;
                    Ok(ThreadLocation::ContributionRequest(cr_type))
                } else {
                    Err(NodecosmosError::InternalServerError(
                        "Invalid ThreadLocation".to_string(),
                    ))
                }
            }
        }
    }
}

impl ContributionRequestThreadLocation {
    pub fn notification_text(&self) -> &str {
        match self {
            ContributionRequestThreadLocation::MainThread => "commented on the main contribution request thread",
            ContributionRequestThreadLocation::ObjectDescription => "commented on the object description",
            ContributionRequestThreadLocation::Node => "commented on the node",
            ContributionRequestThreadLocation::Flow => "commented on the flow",
            ContributionRequestThreadLocation::FlowStep => "commented on the flow step",
            ContributionRequestThreadLocation::InputOutput => "commented on the input/output",
        }
    }
}

#[charybdis_model(
    table_name = comment_threads,
    partition_keys = [branch_id],
    clustering_keys = [object_id, id],
)]
#[derive(Serialize, Deserialize, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CommentThread {
    pub branch_id: Uuid,
    pub object_id: Uuid,
    #[serde(default)]
    pub id: Uuid,
    #[serde(default)]
    pub root_id: Uuid,
    pub title: Text,
    pub author_id: Option<Uuid>,
    pub author: Option<Profile>,
    pub object_type: Text,
    pub thread_location: Text,
    pub line_number: Option<Int>,
    pub line_content: Option<Text>,
    pub participant_ids: Option<Set<Uuid>>,

    #[serde(default = "chrono::Utc::now")]
    pub created_at: Timestamp,

    #[serde(default = "chrono::Utc::now")]
    pub updated_at: Timestamp,

    #[charybdis(ignore)]
    #[serde(skip)]
    pub branch: Option<Branch>,

    #[charybdis(ignore)]
    #[serde(skip)]
    pub node: Option<Node>,
}

impl Callbacks for CommentThread {
    type Extension = RequestData;
    type Error = NodecosmosError;

    async fn before_insert(&mut self, _db_session: &CachingSession, data: &RequestData) -> Result<(), Self::Error> {
        let now = chrono::Utc::now();

        self.author_id = Some(data.current_user.id);
        self.author = Some(Profile::init_from_current_user(&data.current_user));

        match self.thread_location() {
            Ok(ThreadLocation::ContributionRequest(ContributionRequestThreadLocation::MainThread)) => {
                self.id = self.branch_id;
            }
            _ => {
                self.id = Uuid::new_v4();
            }
        }

        self.created_at = now;
        self.updated_at = now;

        Ok(())
    }

    async fn after_insert(&mut self, _session: &CachingSession, data: &RequestData) -> Result<(), Self::Error> {
        NodeCounter::increment_thread_count(data, self.branch_id, self.object_id).await?;

        Ok(())
    }

    async fn after_delete(&mut self, session: &CachingSession, data: &RequestData) -> Result<(), Self::Error> {
        Comment::delete_by_branch_id_and_thread_id(self.branch_id, self.id)
            .execute(session)
            .await?;

        NodeCounter::decrement_thread_count(data, self.branch_id, self.object_id).await?;

        Ok(())
    }
}

impl CommentThread {
    pub async fn branch(&mut self, db_session: &CachingSession) -> Result<&mut Branch, NodecosmosError> {
        match self.thread_location()? {
            ThreadLocation::ContributionRequest(..) => {
                if self.branch.is_none() {
                    let branch = Branch::find_by_id(self.branch_id).execute(db_session).await?;
                    self.branch = Some(branch);
                }

                self.branch
                    .as_mut()
                    .ok_or_else(|| NodecosmosError::NotFound("Branch not found".to_string()))
            }
            _ => {
                return Err(NodecosmosError::InternalServerError(
                    "Branch not found for non-ContributionRequest thread".to_string(),
                ));
            }
        }
    }

    pub async fn node(&mut self, db_session: &CachingSession) -> Result<&mut Node, NodecosmosError> {
        match self.thread_location()? {
            ThreadLocation::Thread => {
                if self.node.is_none() {
                    let node = Node::find_by_branch_id_and_id(self.branch_id, self.object_id)
                        .execute(db_session)
                        .await?;
                    self.node = Some(node);
                }

                self.node
                    .as_mut()
                    .ok_or_else(|| NodecosmosError::NotFound("Node not found".to_string()))
            }
            _ => {
                return Err(NodecosmosError::InternalServerError(
                    "Node not found for non-Node thread".to_string(),
                ));
            }
        }
    }

    pub async fn delete_if_no_comments(&self, db_session: &CachingSession) {
        let comment_res = PkComment::maybe_find_first_by_branch_id_and_thread_id(self.branch_id, self.id)
            .execute(db_session)
            .await;

        match comment_res {
            Ok(Some(_)) => return,
            Ok(None) => {
                let res = self.delete().execute(db_session).await;

                if let Err(e) = res {
                    error!("Error while deleting thread: {}", e);
                }
            }
            Err(e) => {
                error!("Error while checking for comments: {}", e);
                return;
            }
        }
    }

    pub fn thread_object_type(&self) -> Result<ThreadObjectType, NodecosmosError> {
        ThreadObjectType::from_str(&self.object_type)
            .map_err(|e| NodecosmosError::NotFound(format!("Error getting object_type {}: {}", self.object_type, e)))
    }

    pub fn thread_location(&self) -> Result<ThreadLocation, NodecosmosError> {
        ThreadLocation::from_str(&self.thread_location).map_err(|e| {
            NodecosmosError::NotFound(format!(
                "Error getting thread_location '{}': {}",
                self.thread_location, e
            ))
        })
    }
}
