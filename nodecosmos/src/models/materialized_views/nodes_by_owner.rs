use crate::api::current_user::OptCurrentUser;
use crate::errors::NodecosmosError;
use charybdis::macros::charybdis_view_model;
use charybdis::operations::Find;
use charybdis::stream::CharybdisModelStream;
use charybdis::types::{Boolean, Text, Uuid};
use scylla::client::caching_session::CachingSession;
use serde::{Deserialize, Serialize};

#[charybdis_view_model(
    table_name = nodes_by_owner,
    base_table = nodes,
    partition_keys = [owner_id],
    clustering_keys = [id, branch_id]
)]
#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct NodesByOwner {
    pub owner_id: Uuid,
    pub id: Uuid,
    pub branch_id: Uuid,
    pub root_id: Uuid,
    pub title: Text,
    pub is_root: Boolean,
    pub is_public: Boolean,
}

impl NodesByOwner {
    pub async fn root_nodes(
        db_session: &CachingSession,
        opt_cu: &OptCurrentUser,
        owner_id: Uuid,
    ) -> Result<CharybdisModelStream<Self>, NodecosmosError> {
        let nodes = if opt_cu.0.as_ref().is_some_and(|cu| cu.id == owner_id) {
            // list all nodes regardless of visibility
            NodesByOwner::find(
                find_nodes_by_owner_query!("owner_id = ? AND is_root = ? ALLOW FILTERING"),
                (owner_id, true),
            )
            .execute(db_session)
            .await?
        } else {
            // list only public nodes
            NodesByOwner::find(
                find_nodes_by_owner_query!("owner_id = ? AND is_root = ? AND is_public = ? ALLOW FILTERING"),
                (owner_id, true, true),
            )
            .execute(db_session)
            .await?
        };

        Ok(nodes)
    }
}
