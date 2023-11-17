use crate::errors::NodecosmosError;
use crate::models::node::Node;
use crate::models::versioned_node::{VersionedNode, VersionedNodePluckable};
use crate::models::versioned_node_ancestors::VersionedNodeAncestors;
use crate::models::versioned_node_descendants::VersionedNodeDescendants;
use charybdis::batch::CharybdisModelBatch;
use charybdis::operations::{Find, Insert};
use charybdis::types::Uuid;
use chrono::Utc;
use scylla::CachingSession;
use std::collections::HashMap;

pub enum NodeChange {
    Title(String),
    Description(Uuid),
    Workflow(Uuid),
    Parent(Uuid),
    OrderIndex(f64),
    Ancestors(Uuid),
    Descendants(Uuid),
}

impl VersionedNode {
    pub async fn handle_creation(session: &CachingSession, node: &Node, user_id: Uuid) -> Result<(), NodecosmosError> {
        let versioned_ancestors = VersionedNodeAncestors::new(node);
        versioned_ancestors.insert(session).await?;

        let now = Utc::now();

        let node_version = VersionedNode {
            node_id: node.id,
            id: Uuid::new_v4(),
            title: node.title.clone(),
            versioned_description_id: None,
            versioned_ancestors_id: versioned_ancestors.id,
            versioned_descendants_id: None,
            versioned_workflow_id: None,
            order_index: 0.0,
            parent_id: node.parent_id,
            created_at: Some(now),
            updated_at: Some(now),
            user_id: Some(user_id),
        };

        node_version.insert(session).await?;
        node_version.create_new_ancestor_version(session, user_id).await?;

        Ok(())
    }

    pub async fn handle_change(
        session: &CachingSession,
        node: &Node,
        changes: Vec<NodeChange>,
        user_id: Uuid,
    ) -> Result<(), NodecosmosError> {
        let mut new_version = VersionedNode::init_from_latest(session, &node.id).await?;
        new_version.user_id = Some(user_id);

        for attribute in changes {
            match attribute {
                NodeChange::Title(title) => {
                    new_version.title = title;
                }
                NodeChange::Description(versioned_description_id) => {
                    new_version.versioned_description_id = Some(versioned_description_id);
                }
                NodeChange::Workflow(versioned_workflow_id) => {
                    new_version.versioned_workflow_id = Some(versioned_workflow_id);
                }
                NodeChange::Parent(parent_id) => {
                    new_version.parent_id = Some(parent_id);
                }
                NodeChange::OrderIndex(order_index) => {
                    new_version.order_index = order_index;
                }
                NodeChange::Ancestors(versioned_ancestors_id) => {
                    new_version.versioned_ancestors_id = versioned_ancestors_id;
                }
                NodeChange::Descendants(versioned_descendants_id) => {
                    new_version.versioned_descendants_id = Some(versioned_descendants_id);
                }
            }
        }

        new_version.insert(session).await?;
        new_version.create_new_ancestor_version(session, user_id).await?;

        Ok(())
    }

    pub async fn handle_deletion(session: &CachingSession, node: &Node, user_id: Uuid) -> Result<(), NodecosmosError> {
        let new_version = VersionedNode::init_from_latest(session, &node.id).await?;
        new_version
            .create_new_ancestors_version_without_current_node(session, user_id)
            .await?;

        Ok(())
    }

    async fn create_new_ancestor_version(
        &self,
        session: &CachingSession,
        user_id: Uuid,
    ) -> Result<(), NodecosmosError> {
        let mut batch = CharybdisModelBatch::unlogged();

        let ancestors = self.latest_ancestors(session).await?;
        let vnd_ids = ancestors.pluck_versioned_descendants_id();
        let grouped_v_node_descendants = VersionedNodeDescendants::find_grouped_by_node_id(session, vnd_ids).await?;

        for ancestor in ancestors {
            let default_v_node_descendants = Default::default();
            let v_node_descendant = grouped_v_node_descendants
                .get(&ancestor.node_id)
                .unwrap_or_else(|| &default_v_node_descendants);

            let mut descendant_version_by_id = v_node_descendant.descendant_version_by_id.clone();
            descendant_version_by_id.insert(self.node_id, self.id);

            let new_v_node_descendant = VersionedNodeDescendants::new(ancestor.node_id, descendant_version_by_id);
            let mut new_v_node = VersionedNode::init_from(&ancestor).await?;
            new_v_node.versioned_descendants_id = Some(new_v_node_descendant.id);
            new_v_node.user_id = Some(user_id);

            batch.append_insert(&new_v_node_descendant)?;
            batch.append_insert(&new_v_node)?;
        }

        batch.execute(session).await?;

        Ok(())
    }

    async fn create_new_ancestors_version_without_current_node(
        &self,
        session: &CachingSession,
        user_id: Uuid,
    ) -> Result<(), NodecosmosError> {
        let mut batch = CharybdisModelBatch::unlogged();
        let mut descendant_version_by_id = HashMap::new();

        if let Some(ver_desc_id) = self.versioned_descendants_id {
            let v_descendants = VersionedNodeDescendants::find_by_primary_key_value(session, (ver_desc_id,)).await?;
            descendant_version_by_id = v_descendants.descendant_version_by_id;
        }

        let ancestors = self.latest_ancestors(session).await?;
        let vnd_ids = ancestors.pluck_versioned_descendants_id();
        let grouped_v_node_descendants = VersionedNodeDescendants::find_grouped_by_node_id(session, vnd_ids).await?;

        for ancestor in ancestors {
            let default_v_node_descendants = Default::default();
            let v_node_descendant = grouped_v_node_descendants
                .get(&ancestor.node_id)
                .unwrap_or_else(|| &default_v_node_descendants);

            // remove the current node and it's descendants from the ancestor
            let mut anc_descendant_version_by_id = v_node_descendant.descendant_version_by_id.clone();
            anc_descendant_version_by_id
                .retain(|k, _| k != &self.node_id && !&descendant_version_by_id.contains_key(k));

            let new_v_node_descendant = VersionedNodeDescendants::new(ancestor.node_id, anc_descendant_version_by_id);
            let mut new_v_node = VersionedNode::init_from(&ancestor).await?;
            new_v_node.versioned_descendants_id = Some(new_v_node_descendant.id);
            new_v_node.user_id = Some(user_id);

            batch.append_insert(&new_v_node_descendant)?;
            batch.append_insert(&new_v_node)?;
        }

        batch.execute(session).await?;

        Ok(())
    }
}
