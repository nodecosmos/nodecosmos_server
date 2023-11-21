pub mod create;
pub mod pluckable;
pub mod reorder_handler;

use crate::constants::MAX_PARALLEL_REQUESTS;
use crate::errors::NodecosmosError;
use crate::models::versioned_node_descendants::VersionedNodeDescendantIds;
use crate::models::versioned_node_tree_position::VersionedNodeTreePosition;
use charybdis::errors::CharybdisError;
use charybdis::macros::charybdis_model;
use charybdis::types::{Text, Timestamp, Uuid};
use chrono::Utc;
use scylla::CachingSession;
use serde::{Deserialize, Serialize};

#[charybdis_model(
    table_name = versioned_nodes,
    partition_keys = [node_id],
    clustering_keys = [branch_id, created_at, id],
    table_options = r#"
        CLUSTERING ORDER BY (created_at DESC)
    "#
)]
#[derive(Serialize, Deserialize, Default, Clone)]
pub struct VersionedNode {
    #[serde(rename = "nodeId")]
    pub node_id: Uuid,

    #[serde(rename = "branchId")]
    pub branch_id: Uuid,

    #[serde(rename = "createdAt")]
    pub created_at: Timestamp,

    pub id: Uuid,

    #[serde(rename = "userId")]
    pub user_id: Option<Uuid>,

    #[serde(rename = "nodeTitle")]
    pub node_title: Text,

    #[serde(rename = "nodeCreatorId")]
    pub node_creator_id: Option<Uuid>,

    #[serde(rename = "nodeCreatedAt")]
    pub node_created_at: Option<Timestamp>,

    #[serde(rename = "nodeDeletedAt")]
    pub node_deleted_at: Option<Timestamp>,

    #[serde(rename = "versionedDescriptionId")]
    pub versioned_description_id: Option<Uuid>,

    #[serde(rename = "versionedTreePositionId")]
    pub versioned_tree_position_id: Uuid,

    #[serde(rename = "versionedNodeDescendantsId")]
    pub versioned_descendants_id: Option<Uuid>,

    #[serde(rename = "versionedWorkflowId")]
    pub versioned_workflow_id: Option<Uuid>,
}

impl VersionedNode {
    async fn find_first_by_node_id<'a>(
        session: &CachingSession,
        node_id: &Uuid,
        branch_id: &Uuid,
    ) -> Result<Self, CharybdisError> {
        let res = find_first_versioned_node!(
            session,
            "node_id = ? AND branch_id = ? OR branch_id = ? LIMIT 1",
            (node_id, branch_id, node_id)
        )
        .await?;

        Ok(res)
    }

    pub fn init_from(v_node: &Self, branch_id: Uuid) -> Self {
        let mut v_node = v_node.clone();
        v_node.id = Uuid::new_v4();
        v_node.created_at = Utc::now();
        v_node.branch_id = branch_id;

        v_node
    }

    pub async fn init_from_latest(
        session: &CachingSession,
        node_id: &Uuid,
        branch_id: &Uuid,
    ) -> Result<Self, NodecosmosError> {
        let mut v_node = VersionedNode::find_first_by_node_id(session, node_id, branch_id).await?;
        v_node.id = Uuid::new_v4();
        v_node.created_at = Utc::now();
        v_node.branch_id = *branch_id;

        Ok(v_node)
    }

    async fn find_latest_by_node_ids(
        &self,
        session: &CachingSession,
        ids: &Vec<Uuid>,
    ) -> Result<Vec<Self>, NodecosmosError> {
        let mut versioned_nodes = vec![];

        let ids_chunks = ids.chunks(MAX_PARALLEL_REQUESTS);
        for ids_chunk in ids_chunks {
            let mut futures = vec![];

            for ancestor_id in ids_chunk {
                let future = VersionedNode::find_first_by_node_id(session, ancestor_id, &self.branch_id);
                futures.push(future);
            }

            let futures_res = futures::future::join_all(futures).await;

            for res in futures_res {
                let versioned_node = res?;
                versioned_nodes.push(versioned_node);
            }
        }

        return Ok(versioned_nodes);
    }

    async fn find_previous_version(&self, session: &CachingSession) -> Result<Self, CharybdisError> {
        let res = find_first_versioned_node!(
            session,
            "node_id = ? AND created_at < ? LIMIT 1",
            (self.node_id, self.created_at)
        )
        .await?;

        Ok(res)
    }

    pub async fn versioned_tree_position(
        &mut self,
        session: &CachingSession,
    ) -> Result<VersionedNodeTreePosition, NodecosmosError> {
        let res = VersionedNodeTreePosition::find_by_id(session, self.versioned_tree_position_id).await?;

        Ok(res)
    }

    pub async fn versioned_descendant_ids(
        &self,
        session: &CachingSession,
    ) -> Result<Option<VersionedNodeDescendantIds>, NodecosmosError> {
        if let Some(versioned_descendants_id) = self.versioned_descendants_id {
            let res = VersionedNodeDescendantIds::find_by_id(session, versioned_descendants_id).await?;

            return Ok(Some(res));
        }

        Ok(None)
    }

    pub async fn latest_versioned_ancestors(&mut self, session: &CachingSession) -> Result<Vec<Self>, NodecosmosError> {
        let vtp = self.versioned_tree_position(session).await?;

        if let Some(ancestor_ids) = &vtp.ancestor_ids {
            return self.find_latest_by_node_ids(session, ancestor_ids).await;
        }

        Ok(vec![])
    }

    pub async fn latest_versioned_descendants(&self, session: &CachingSession) -> Result<Vec<Self>, NodecosmosError> {
        let vd = self.versioned_descendant_ids(session).await?;

        if let Some(vd) = vd {
            let descendant_ids = vd.descendant_version_by_id.keys().cloned().collect();

            return self.find_latest_by_node_ids(session, &descendant_ids).await;
        }

        Ok(vec![])
    }
}
