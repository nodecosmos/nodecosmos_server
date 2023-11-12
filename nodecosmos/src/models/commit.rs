mod callbacks;

use crate::actions::types::ActionTypes;
use crate::errors::NodecosmosError;
use charybdis::macros::charybdis_model;
use charybdis::types::{Timestamp, Uuid};
use scylla::CachingSession;
use serde::{Deserialize, Serialize};
use std::rc::Rc;

pub type CommitTypes = ActionTypes;

#[charybdis_model(
    table_name = commits,
    partition_keys = [node_id],
    clustering_keys = [created_at, id],
    local_secondary_indexes = [
      ([node_id], [id])
    ],
    table_options = r#"
        CLUSTERING ORDER BY (created_at DESC)
    "#
)]
#[derive(Serialize, Deserialize, Default)]
pub struct Commit {
    #[serde(rename = "nodeId")]
    pub node_id: Uuid,

    pub contribution_request_id: Option<Uuid>,
    pub id: Uuid,

    #[serde(rename = "prevId")]
    pub prev_id: Option<Uuid>,

    #[serde(rename = "createdAt")]
    pub created_at: Option<Timestamp>,

    #[serde(rename = "updatedAt")]
    pub updated_at: Option<Timestamp>,

    #[charybdis(ignore)]
    #[serde(skip)]
    pub prev_commit: Rc<Option<Commit>>,
}

impl Commit {
    pub async fn prev_commit(&mut self, session: &CachingSession) -> Result<&Rc<Option<Commit>>, NodecosmosError> {
        if self.prev_commit.is_none() {
            if let Some(prev_id) = self.prev_id {
                let pc = find_one_commit!(session, "node_id = ? AND id = ?", (self.node_id, prev_id))
                    .await
                    .ok();

                self.prev_commit = Rc::new(pc);
            } else {
                self.prev_commit = Rc::new(None);
            }
        }

        return Ok(&self.prev_commit);
    }
}
