use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::node::reorder::reorder_data::ReorderData;
use crate::models::node::Node;
use crate::models::versioned_node::pluckable::VersionedNodePluckable;
use crate::models::versioned_node::reorder_handler::VersionReorderHandler;
use crate::models::versioned_node::VersionedNode;
use crate::models::versioned_node_ancestor_ids::VersionedNodeAncestorIds;
use crate::models::versioned_node_descendants_by_id::VersionedNodeDescendantIds;
use charybdis::batch::CharybdisModelBatch;
use charybdis::operations::{Find, Insert};
use charybdis::types::Uuid;
use chrono::Utc;
use scylla::CachingSession;
use std::collections::HashMap;

pub type RemovedIds = Vec<Uuid>;
pub type AddedIds = Vec<Uuid>;
pub type NewDescendantVersionById = HashMap<Uuid, Uuid>;

pub enum NodeChange {
    Title(String),
    Description(Uuid),
    Workflow(Uuid),
    Parent(Uuid),
    OrderIndex(f64),
    Ancestors(RemovedIds, AddedIds),
    Descendants(RemovedIds, Option<NewDescendantVersionById>),
}

impl VersionedNode {
    pub async fn handle_creation(session: &CachingSession, node: &Node, user_id: Uuid) -> Result<(), NodecosmosError> {
        let versioned_ancestors = VersionedNodeAncestorIds::from_node(node);
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
            deleted_at: None,
            user_id: Some(user_id),
            creator_id: node.creator_id,
        };

        node_version.insert(session).await?;
        node_version.create_new_ancestors_versions(session, user_id).await?;

        Ok(())
    }

    pub async fn handle_change(
        session: &CachingSession,
        node_id: Uuid,
        user_id: Uuid,
        changes: &Vec<NodeChange>,
        update_ancestors: bool,
    ) -> Result<VersionedNode, NodecosmosError> {
        let mut new_version = VersionedNode::init_from_latest(session, &node_id).await?;
        new_version.user_id = Some(user_id);

        for attribute in changes {
            match attribute {
                NodeChange::Title(title) => {
                    new_version.title = title.clone();
                }
                NodeChange::Description(versioned_description_id) => {
                    new_version.versioned_description_id = Some(*versioned_description_id);
                }
                NodeChange::Workflow(versioned_workflow_id) => {
                    new_version.versioned_workflow_id = Some(*versioned_workflow_id);
                }
                NodeChange::Parent(parent_id) => {
                    new_version.parent_id = Some(*parent_id);
                }
                NodeChange::OrderIndex(order_index) => {
                    new_version.order_index = *order_index;
                }
                NodeChange::Ancestors(removed_ids, added_ids) => {
                    let old_versioned_ancestors = new_version.versioned_ancestor_ids(session).await?;
                    let mut ancestor_ids = old_versioned_ancestors.ancestor_ids.unwrap_or_default();

                    ancestor_ids.retain(|id| !removed_ids.contains(id));
                    ancestor_ids.extend(added_ids);

                    let new_versioned_ancestors = VersionedNodeAncestorIds {
                        id: Uuid::new_v4(),
                        ancestor_ids: Some(ancestor_ids),
                    };

                    new_version.versioned_ancestors_id = new_versioned_ancestors.id;
                }
                NodeChange::Descendants(removed_ids, new_descendant_version_by_id) => {
                    let old_versioned_descendants = new_version.versioned_descendant_ids(session).await?;
                    if let Some(old_versioned_descendants) = old_versioned_descendants {
                        let mut descendant_version_by_id = old_versioned_descendants.descendant_version_by_id;
                        descendant_version_by_id.retain(|id, _| !removed_ids.contains(id));

                        if let Some(new_descendant_version_by_id) = new_descendant_version_by_id {
                            descendant_version_by_id.extend(new_descendant_version_by_id);
                        }

                        let new_versioned_descendants = VersionedNodeDescendantIds {
                            id: Uuid::new_v4(),
                            node_id: node_id.clone(),
                            descendant_version_by_id,
                        };

                        new_version.versioned_descendants_id = Some(new_versioned_descendants.id);
                    }
                }
            }
        }

        new_version.insert(session).await?;

        if update_ancestors {
            new_version.create_new_ancestors_versions(session, user_id).await?;
        }

        Ok(new_version)
    }

    pub async fn handle_deletion(request_data: &RequestData, node: &Node) -> Result<(), NodecosmosError> {
        let mut new_version = VersionedNode::init_from_latest(request_data.db_session(), &node.id).await?;
        new_version.deleted_at = Some(Utc::now());
        new_version.insert(request_data.db_session()).await?;

        new_version
            .create_new_ancestors_versions_without_current_node(
                &request_data.db_session(),
                request_data.current_user.id,
            )
            .await?;

        Ok(())
    }

    pub async fn handle_reorder(request_data: &RequestData, reorder_data: &ReorderData) -> Result<(), NodecosmosError> {
        let session = request_data.app.db_session.clone();
        let current_user_id = request_data.current_user.id;

        let version_handler = VersionReorderHandler::from_reorder_data(session, current_user_id, &reorder_data);

        // create new thread
        tokio::spawn(async move {
            let _ = version_handler.run().await;
        });

        Ok(())
    }

    async fn create_new_ancestors_versions(
        &self,
        session: &CachingSession,
        user_id: Uuid,
    ) -> Result<(), NodecosmosError> {
        let mut batch = CharybdisModelBatch::unlogged();

        let ancestors = self.latest_ancestors(session).await?;
        let vnd_ids = ancestors.pluck_versioned_descendants_id();
        let grouped_v_node_descendants = VersionedNodeDescendantIds::find_grouped_by_node_id(session, vnd_ids).await?;

        for ancestor in ancestors {
            let default_v_node_descendants = Default::default();
            let v_node_descendant = grouped_v_node_descendants
                .get(&ancestor.node_id)
                .unwrap_or_else(|| &default_v_node_descendants);

            let mut descendant_version_by_id = v_node_descendant.descendant_version_by_id.clone();
            descendant_version_by_id.insert(self.node_id, self.id);

            let new_v_node_descendant = VersionedNodeDescendantIds::new(ancestor.node_id, descendant_version_by_id);
            let mut new_v_node = VersionedNode::init_from(&ancestor).await?;
            new_v_node.versioned_descendants_id = Some(new_v_node_descendant.id);
            new_v_node.user_id = Some(user_id);

            batch.append_insert(&new_v_node_descendant)?;
            batch.append_insert(&new_v_node)?;
        }

        batch.execute(session).await?;

        Ok(())
    }

    async fn create_new_ancestors_versions_without_current_node(
        &self,
        session: &CachingSession,
        user_id: Uuid,
    ) -> Result<(), NodecosmosError> {
        let mut batch = CharybdisModelBatch::unlogged();
        let mut descendant_version_by_id = HashMap::new();

        if let Some(ver_desc_id) = self.versioned_descendants_id {
            let v_descendants = VersionedNodeDescendantIds::find_by_primary_key_value(session, (ver_desc_id,)).await?;
            descendant_version_by_id = v_descendants.descendant_version_by_id;
        }

        let ancestors = self.latest_ancestors(session).await?;
        let vnd_ids = ancestors.pluck_versioned_descendants_id();
        let grouped_v_node_descendants = VersionedNodeDescendantIds::find_grouped_by_node_id(session, vnd_ids).await?;

        for ancestor in ancestors {
            let default_v_node_descendants = Default::default();
            let v_node_descendant = grouped_v_node_descendants
                .get(&ancestor.node_id)
                .unwrap_or_else(|| &default_v_node_descendants);

            // remove the current node and it's descendants from the ancestor
            let mut anc_descendant_version_by_id = v_node_descendant.descendant_version_by_id.clone();
            anc_descendant_version_by_id
                .retain(|k, _| k != &self.node_id && !&descendant_version_by_id.contains_key(k));

            let new_v_node_descendant = VersionedNodeDescendantIds::new(ancestor.node_id, anc_descendant_version_by_id);
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
