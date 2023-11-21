use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::node::reorder::reorder_data::ReorderData;
use crate::models::node::Node;
use crate::models::versioned_node::pluckable::VersionedNodePluckable;
use crate::models::versioned_node::reorder_handler::VersionReorderHandler;
use crate::models::versioned_node::VersionedNode;
use crate::models::versioned_node_descendants::VersionedNodeDescendantIds;
use crate::models::versioned_node_tree_position::VersionedNodeTreePosition;
use crate::utils::logger::log_fatal;
use charybdis::batch::CharybdisModelBatch;
use charybdis::operations::Insert;
use charybdis::types::{Double, Uuid};
use chrono::Utc;
use futures::TryFutureExt;
use scylla::CachingSession;
use std::collections::HashMap;

pub struct TreePositionChange {
    pub parent_id: Option<Uuid>,
    pub order_index: Option<Double>,
    pub removed_ancestor_ids: Option<Vec<Uuid>>,
    pub added_ancestor_ids: Option<Vec<Uuid>>,
}

pub type RemovedIds = Vec<Uuid>;
pub type AddedIds = Vec<Uuid>;
pub type NewDescendantVersionById = HashMap<Uuid, Uuid>;

pub enum NodeChange {
    Title(String),
    Description(Uuid),
    Workflow(Uuid),
    TreePosition(TreePositionChange),
    Descendants(Option<RemovedIds>, Option<NewDescendantVersionById>),
}

impl VersionedNode {
    pub async fn handle_creation(session: &CachingSession, node: &Node, user_id: Uuid) -> Result<(), NodecosmosError> {
        let versioned_tree_position = VersionedNodeTreePosition::from_node(node);
        versioned_tree_position.insert(session).await?;

        let now = Utc::now();

        let mut node_version = VersionedNode {
            node_id: node.id,
            branch_id: node.branch_id,
            id: Uuid::new_v4(),
            node_title: node.title.clone(),
            node_creator_id: node.creator_id,
            node_created_at: node.created_at,
            node_deleted_at: None,
            versioned_tree_position_id: versioned_tree_position.id,
            versioned_description_id: None,
            versioned_descendants_id: None,
            versioned_workflow_id: None,
            created_at: now,
            user_id: Some(user_id),
        };

        node_version.insert(session).await?;
        node_version.create_new_ancestors_versions(session).await?;

        Ok(())
    }

    pub async fn handle_change(
        session: &CachingSession,
        node_id: Uuid,
        branch_id: Uuid,
        user_id: Uuid,
        changes: &Vec<NodeChange>,
        update_ancestors: bool,
    ) -> Result<VersionedNode, NodecosmosError> {
        let mut new_version = VersionedNode::init_from_latest(session, &node_id, &branch_id).await?;
        new_version.user_id = Some(user_id);

        for attribute in changes {
            match attribute {
                NodeChange::Title(title) => {
                    new_version.node_title = title.clone();
                }
                NodeChange::Description(versioned_description_id) => {
                    new_version.versioned_description_id = Some(*versioned_description_id);
                }
                NodeChange::Workflow(versioned_workflow_id) => {
                    new_version.versioned_workflow_id = Some(*versioned_workflow_id);
                }
                NodeChange::TreePosition(tree_position_change) => {
                    let old_vtp = new_version.versioned_tree_position(session).await?;
                    let mut new_vtp = VersionedNodeTreePosition::init_from(&old_vtp);

                    if let Some(parent_id) = tree_position_change.parent_id {
                        new_vtp.parent_id = Some(parent_id);
                    }

                    if let Some(order_index) = tree_position_change.order_index {
                        new_vtp.order_index = order_index;
                    }

                    if let Some(removed_ancestor_ids) = &tree_position_change.removed_ancestor_ids {
                        let mut ancestor_ids = new_vtp.ancestor_ids.unwrap_or_default();
                        ancestor_ids.retain(|id| !removed_ancestor_ids.contains(id));
                        new_vtp.ancestor_ids = Some(ancestor_ids);
                    }

                    if let Some(added_ancestor_ids) = &tree_position_change.added_ancestor_ids {
                        let mut ancestor_ids = new_vtp.ancestor_ids.unwrap_or_default();
                        ancestor_ids.extend(added_ancestor_ids);
                        new_vtp.ancestor_ids = Some(ancestor_ids);
                    }
                }
                NodeChange::Descendants(removed_ids, new_descendant_version_by_id) => {
                    let old_versioned_descendants = new_version.versioned_descendant_ids(session).await?;
                    if let Some(old_versioned_descendants) = old_versioned_descendants {
                        let mut descendant_version_by_id = old_versioned_descendants.descendant_version_by_id;

                        if let Some(remove_ids) = removed_ids {
                            for id in remove_ids {
                                descendant_version_by_id.remove(id);
                            }
                        }

                        if let Some(new_descendant_version_by_id) = new_descendant_version_by_id {
                            descendant_version_by_id.extend(new_descendant_version_by_id);
                        }

                        let new_versioned_descendants = VersionedNodeDescendantIds {
                            id: Uuid::new_v4(),
                            node_id,
                            descendant_version_by_id,
                        };

                        new_version.versioned_descendants_id = Some(new_versioned_descendants.id);
                    }
                }
            }
        }

        new_version.insert(session).await?;

        if update_ancestors {
            new_version.create_new_ancestors_versions(session).await?;
        }

        Ok(new_version)
    }

    pub async fn handle_deletion(request_data: &RequestData, node: &Node) -> Result<(), NodecosmosError> {
        let mut new_version =
            VersionedNode::init_from_latest(request_data.db_session(), &node.id, &node.branch_id).await?;
        new_version.node_deleted_at = Some(Utc::now());
        new_version.insert(request_data.db_session()).await?;

        let deleted_versioned_descendants = new_version
            .create_deleted_versioned_descendants(&request_data.db_session())
            .await?;

        new_version
            .create_new_ancestors_versions_for_deleted_node(&request_data.db_session(), &deleted_versioned_descendants)
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

    async fn create_deleted_versioned_descendants(
        &self,
        session: &CachingSession,
    ) -> Result<Vec<Self>, NodecosmosError> {
        let mut res = vec![];
        let lvd_nodes = self.latest_versioned_descendants(session).await?;

        for lvd_nodes_chunk in lvd_nodes.chunks(100) {
            let mut batch = CharybdisModelBatch::unlogged();

            for lvd_node in lvd_nodes_chunk {
                let mut versioned_desc_node = VersionedNode::init_from(lvd_node, self.branch_id);
                versioned_desc_node.node_deleted_at = self.node_deleted_at;

                let _ = batch.append_insert(&versioned_desc_node).map_err(|err| {
                    log_fatal(format!(
                        "Failed to append_insert versioned descendant: {:?}, node_id: {}",
                        &err, versioned_desc_node.node_id,
                    ));
                });

                res.push(versioned_desc_node);
            }

            let _ = batch
                .execute(session)
                .map_err(|err| {
                    log_fatal(format!("Failed to execute batch: {:?}", &err));
                })
                .await;
        }

        Ok(res)
    }

    async fn create_new_ancestors_versions(&mut self, session: &CachingSession) -> Result<(), NodecosmosError> {
        let mut batch = CharybdisModelBatch::unlogged();

        let versioned_ancestors = self.latest_versioned_ancestors(session).await?;
        let vnd_ids = versioned_ancestors.pluck_versioned_descendants_id();
        let grouped_v_node_descendants = VersionedNodeDescendantIds::find_grouped_by_node_id(session, vnd_ids).await?;

        for versioned_ancestor in versioned_ancestors {
            let default_v_node_descendants = Default::default();
            let v_node_descendant = grouped_v_node_descendants
                .get(&versioned_ancestor.node_id)
                .unwrap_or_else(|| &default_v_node_descendants);

            let mut descendant_version_by_id = v_node_descendant.descendant_version_by_id.clone();
            descendant_version_by_id.insert(self.node_id, self.id);

            let new_v_node_descendant =
                VersionedNodeDescendantIds::new(versioned_ancestor.node_id, descendant_version_by_id);
            let mut new_v_node = VersionedNode::init_from(&versioned_ancestor, self.branch_id);
            new_v_node.versioned_descendants_id = Some(new_v_node_descendant.id);
            new_v_node.user_id = self.user_id;

            batch.append_insert(&new_v_node_descendant)?;
            batch.append_insert(&new_v_node)?;
        }

        batch.execute(session).await?;

        Ok(())
    }

    async fn create_new_ancestors_versions_for_deleted_node(
        &mut self,
        session: &CachingSession,
        deleted_versioned_descendants: &Vec<VersionedNode>,
    ) -> Result<(), NodecosmosError> {
        let mut batch = CharybdisModelBatch::unlogged();

        let ancestors = self.latest_versioned_ancestors(session).await?;
        let vnd_ids = ancestors.pluck_versioned_descendants_id();
        let grouped_v_node_descendants = VersionedNodeDescendantIds::find_grouped_by_node_id(session, vnd_ids).await?;

        for ancestor in ancestors {
            let default_v_node_descendants = Default::default();
            let v_node_descendant = grouped_v_node_descendants
                .get(&ancestor.node_id)
                .unwrap_or_else(|| &default_v_node_descendants);

            let mut anc_descendant_version_by_id = v_node_descendant.descendant_version_by_id.clone();
            anc_descendant_version_by_id.insert(self.node_id, self.id);
            deleted_versioned_descendants.iter().for_each(|desc| {
                anc_descendant_version_by_id.insert(desc.node_id, desc.id);
            });

            let new_v_node_descendant = VersionedNodeDescendantIds::new(ancestor.node_id, anc_descendant_version_by_id);
            let mut new_v_node = VersionedNode::init_from(&ancestor, self.branch_id);
            new_v_node.branch_id = self.branch_id;
            new_v_node.versioned_descendants_id = Some(new_v_node_descendant.id);
            new_v_node.user_id = self.user_id;

            batch.append_insert(&new_v_node_descendant)?;
            batch.append_insert(&new_v_node)?;
        }

        batch.execute(session).await?;

        Ok(())
    }
}
