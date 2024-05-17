use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::node::Node;
use charybdis::callbacks::Callbacks;
use charybdis::macros::charybdis_model;
use charybdis::types::{Boolean, Text, Uuid};
use scylla::CachingSession;
use serde::Serialize;

#[derive(strum_macros::Display, strum_macros::EnumString)]
pub enum InvitationStatus {
    Created,
    Seen,
    Accepted,
    Rejected,
}

#[charybdis_model(
    table_name = invitations,
    partition_keys = [user_id],
    clustering_keys = [id],
)]
#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Invitation {
    pub id: Uuid,
    pub user_id: Uuid,
    pub email: Text,
    pub seen: Boolean,
    pub status: Text,
    pub node_id: Uuid,
    pub branch_id: Uuid,

    #[serde(skip)]
    #[charybdis(ignore)]
    pub node: Option<Node>,
}

impl Callbacks for Invitation {
    type Error = NodecosmosError;
    type Extension = RequestData;

    async fn before_insert(&mut self, _db_session: &CachingSession, data: &RequestData) -> Result<(), NodecosmosError> {
        self.status = InvitationStatus::Created.to_string();
        Ok(())
    }
}

impl Invitation {
    pub async fn node(&mut self, db_session: &CachingSession) -> Result<&mut Node, NodecosmosError> {
        if self.node.is_none() {
            let node = Node::find_by_branch_id_and_id(self.branch_id, self.node_id)
                .execute(db_session)
                .await?;
            self.node = Some(node);
        }

        Ok(self.node.as_mut().unwrap())
    }
}
