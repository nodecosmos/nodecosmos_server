use crate::constants::MAX_PARALLEL_REQUESTS;
use crate::errors::NodecosmosError;
use crate::models::node::reorder::reorder_data::ReorderData;
use crate::models::versioned_node::create::{NewDescendantVersionById, NodeChange, TreePositionChange};
use crate::models::versioned_node::VersionedNode;
use crate::utils::logger::log_fatal;
use charybdis::types::Uuid;
use futures::TryFutureExt;
use scylla::CachingSession;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Clone)]
pub struct VersionReorderHandler {
    session: Arc<CachingSession>,
    current_user_id: Uuid,
    node_id: Uuid,
    descendant_ids: Vec<Uuid>,
    new_parent_id: Uuid,
    new_order_index: f64,
    parent_changed: bool,
    removed_ancestor_ids: Vec<Uuid>,
    added_ancestor_ids: Vec<Uuid>,
}

impl VersionReorderHandler {
    pub fn from_reorder_data(session: Arc<CachingSession>, current_user_id: Uuid, reorder_data: &ReorderData) -> Self {
        Self {
            session,
            current_user_id,
            node_id: reorder_data.node.id,
            descendant_ids: reorder_data.descendant_ids.clone(),
            new_parent_id: reorder_data.new_parent.id,
            new_order_index: reorder_data.new_order_index,
            parent_changed: reorder_data.parent_changed(),
            removed_ancestor_ids: reorder_data.removed_ancestor_ids.clone(),
            added_ancestor_ids: reorder_data.added_ancestor_ids.clone(),
        }
    }

    pub async fn run(&self) -> Result<(), NodecosmosError> {
        // if parent is changed we will update the ancestors in the next step
        let new_node_version = self.create_new_node_version(!self.parent_changed).await?;

        if self.parent_changed {
            let mut descendant_version_by_id = HashMap::new();
            descendant_version_by_id.insert(new_node_version.node_id, new_node_version.id);
            let ext_desc_ver = self.create_new_version_for_descendants().await.map_err(|e| {
                log_fatal(format!("create_new_version_for_descendants: {:?}", e));
                e
            })?;

            descendant_version_by_id.extend(ext_desc_ver);

            self.create_new_version_for_removed_ancestors().await.map_err(|e| {
                log_fatal(format!("create_new_version_for_removed_ancestors: {:?}", e));

                e
            })?;

            self.create_new_version_for_added_ancestors(descendant_version_by_id)
                .await
                .map_err(|e| {
                    log_fatal(format!("create_new_version_for_removed_ancestors: {:?}", e));

                    e
                })?;
        }

        Ok(())
    }

    async fn create_new_node_version(&self, update_ancestors: bool) -> Result<VersionedNode, NodecosmosError> {
        let tpc = TreePositionChange {
            parent_id: Some(self.new_parent_id),
            order_index: Some(self.new_order_index),
            removed_ancestor_ids: Some(self.removed_ancestor_ids.clone()),
            added_ancestor_ids: Some(self.added_ancestor_ids.clone()),
        };

        let changes = vec![NodeChange::TreePosition(tpc)];

        let new_node_version = VersionedNode::handle_change(
            &self.session,
            self.node_id,
            self.current_user_id,
            &changes,
            update_ancestors,
        )
        .await
        .map_err(|err| {
            log_fatal(format!("create_new_node_version: {:?}", err));
            return err;
        })?;

        Ok(new_node_version)
    }

    async fn create_new_version_for_descendants(&self) -> Result<NewDescendantVersionById, NodecosmosError> {
        let mut descendant_version_by_id = HashMap::new();
        let tpc = TreePositionChange {
            parent_id: None,
            order_index: None,
            removed_ancestor_ids: Some(self.removed_ancestor_ids.clone()),
            added_ancestor_ids: Some(self.added_ancestor_ids.clone()),
        };

        let changes = vec![NodeChange::TreePosition(tpc)];

        for id_chunk in self.descendant_ids.chunks(MAX_PARALLEL_REQUESTS) {
            let mut futures = vec![];

            for id in id_chunk {
                let future = VersionedNode::handle_change(&self.session, *id, self.current_user_id, &changes, false)
                    .map_err(|e| {
                        log_fatal(format!("create_new_version_for_descendants: {:?}", e));
                    });

                futures.push(future);
            }

            // change ancestors in batches of 25 parallel requests
            let results = futures::future::join_all(futures).await;
            for result in results {
                match result {
                    Ok(result) => {
                        descendant_version_by_id.insert(result.node_id, result.id);
                    }
                    _ => {}
                }
            }
        }

        Ok(descendant_version_by_id)
    }

    async fn create_new_version_for_removed_ancestors(&self) -> Result<(), NodecosmosError> {
        let mut removed_descendant_ids = vec![self.node_id];
        removed_descendant_ids.extend(self.descendant_ids.clone());
        let changes = vec![NodeChange::Descendants(Some(removed_descendant_ids.clone()), None)];

        for id_chunk in self.removed_ancestor_ids.chunks(MAX_PARALLEL_REQUESTS) {
            let mut futures = vec![];

            for ancestor_id in id_chunk {
                let f =
                    VersionedNode::handle_change(&self.session, *ancestor_id, self.current_user_id, &changes, false)
                        .map_err(|e| {
                            log_fatal(format!("create_new_version_for_descendants: {:?}", e));
                        });

                futures.push(f);
            }

            // change descendants in batches of 25 parallel requests
            let _ = futures::future::join_all(futures).await;
        }

        Ok(())
    }

    async fn create_new_version_for_added_ancestors(
        &self,
        dv_by_id: NewDescendantVersionById,
    ) -> Result<(), NodecosmosError> {
        let changes = vec![NodeChange::Descendants(None, Some(dv_by_id.clone()))];

        for id_chunk in self.added_ancestor_ids.chunks(MAX_PARALLEL_REQUESTS) {
            let mut futures = vec![];

            for ancestor_id in id_chunk {
                let f =
                    VersionedNode::handle_change(&self.session, *ancestor_id, self.current_user_id, &changes, false)
                        .map_err(|e| {
                            log_fatal(format!("create_new_version_for_descendants: {:?}", e));
                        });

                futures.push(f);
            }

            // change ancestors in batches of 25 parallel requests
            let _ = futures::future::join_all(futures).await;
        }

        Ok(())
    }
}
