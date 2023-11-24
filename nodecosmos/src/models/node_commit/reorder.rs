use crate::constants::MAX_PARALLEL_REQUESTS;
use crate::errors::NodecosmosError;
use crate::models::node::reorder::reorder_data::ReorderData;
use crate::models::node_commit::create::{NewDescendantCommitById, NodeChange, TreePositionChange};
use crate::models::node_commit::NodeCommit;
use crate::utils::logger::log_fatal;
use charybdis::types::Uuid;
use futures::TryFutureExt;
use scylla::CachingSession;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Clone)]
pub struct ReorderCommit {
    session: Arc<CachingSession>,
    current_user_id: Uuid,
    node_id: Uuid,
    branch_id: Uuid,
    descendant_ids: Vec<Uuid>,
    new_parent_id: Uuid,
    new_order_index: f64,
    parent_changed: bool,
    removed_ancestor_ids: Vec<Uuid>,
    added_ancestor_ids: Vec<Uuid>,
}

impl ReorderCommit {
    pub fn from_reorder_data(session: Arc<CachingSession>, current_user_id: Uuid, reorder_data: &ReorderData) -> Self {
        Self {
            session,
            current_user_id,
            node_id: reorder_data.node.id,
            branch_id: reorder_data.branch_id,
            descendant_ids: reorder_data.descendant_ids.clone(),
            new_parent_id: reorder_data.new_parent.id,
            new_order_index: reorder_data.new_order_index,
            parent_changed: reorder_data.parent_changed(),
            removed_ancestor_ids: reorder_data.removed_ancestor_ids.clone(),
            added_ancestor_ids: reorder_data.added_ancestor_ids.clone(),
        }
    }

    pub async fn create(&self) -> Result<(), NodecosmosError> {
        let new_node_commit = self.create_new_node_commit().await?;

        if self.parent_changed {
            let mut descendant_node_commit_id_by_id = HashMap::new();
            descendant_node_commit_id_by_id.insert(new_node_commit.node_id, new_node_commit.id);
            let ext_desc_ver = self.create_descendants_commits().await.map_err(|e| {
                log_fatal(format!("create_descendants_commits: {:?}", e));
                e
            })?;

            descendant_node_commit_id_by_id.extend(ext_desc_ver);

            self.create_commits_for_removed_ancestors().await.map_err(|e| {
                log_fatal(format!("create_commits_for_removed_ancestors: {:?}", e));

                e
            })?;

            self.create_commits_for_added_ancestors(descendant_node_commit_id_by_id)
                .await
                .map_err(|e| {
                    log_fatal(format!("create_commits_for_removed_ancestors: {:?}", e));

                    e
                })?;
        }

        Ok(())
    }

    async fn create_new_node_commit(&self) -> Result<NodeCommit, NodecosmosError> {
        let tpc = TreePositionChange {
            parent_id: Some(self.new_parent_id),
            order_index: Some(self.new_order_index),
            removed_ancestor_ids: Some(self.removed_ancestor_ids.clone()),
            added_ancestor_ids: Some(self.added_ancestor_ids.clone()),
        };

        let changes = vec![NodeChange::TreePosition(tpc)];

        // if parent is changed we will create ancestors commits in the next step
        let new_node_commit = NodeCommit::handle_change(
            &self.session,
            self.node_id,
            self.branch_id,
            self.current_user_id,
            &changes,
            !self.parent_changed,
        )
        .await
        .map_err(|err| {
            log_fatal(format!("create_new_node_commit: {:?}", err));
            return err;
        })?;

        Ok(new_node_commit)
    }

    async fn create_descendants_commits(&self) -> Result<NewDescendantCommitById, NodecosmosError> {
        let mut descendant_node_commit_id_by_id = HashMap::new();
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
                let future = NodeCommit::handle_change(
                    &self.session,
                    *id,
                    self.branch_id,
                    self.current_user_id,
                    &changes,
                    false,
                )
                .map_err(|e| {
                    log_fatal(format!("create_descendants_commits: {:?}", e));
                });

                futures.push(future);
            }

            // change ancestors in batches of 25 parallel requests
            let results = futures::future::join_all(futures).await;
            for result in results {
                match result {
                    Ok(result) => {
                        descendant_node_commit_id_by_id.insert(result.node_id, result.id);
                    }
                    _ => {}
                }
            }
        }

        Ok(descendant_node_commit_id_by_id)
    }

    async fn create_commits_for_removed_ancestors(&self) -> Result<(), NodecosmosError> {
        let mut removed_descendant_ids = vec![self.node_id];
        removed_descendant_ids.extend(self.descendant_ids.clone());
        let changes = vec![NodeChange::Descendants(Some(removed_descendant_ids.clone()), None)];

        for id_chunk in self.removed_ancestor_ids.chunks(MAX_PARALLEL_REQUESTS) {
            let mut futures = vec![];

            for ancestor_id in id_chunk {
                let f = NodeCommit::handle_change(
                    &self.session,
                    *ancestor_id,
                    self.branch_id,
                    self.current_user_id,
                    &changes,
                    false,
                )
                .map_err(|e| {
                    log_fatal(format!("create_descendants_commits: {:?}", e));
                });

                futures.push(f);
            }

            // change descendants in batches of 25 parallel requests
            let _ = futures::future::join_all(futures).await;
        }

        Ok(())
    }

    async fn create_commits_for_added_ancestors(
        &self,
        dv_by_id: NewDescendantCommitById,
    ) -> Result<(), NodecosmosError> {
        let changes = vec![NodeChange::Descendants(None, Some(dv_by_id.clone()))];

        for id_chunk in self.added_ancestor_ids.chunks(MAX_PARALLEL_REQUESTS) {
            let mut futures = vec![];

            for ancestor_id in id_chunk {
                let f = NodeCommit::handle_change(
                    &self.session,
                    *ancestor_id,
                    self.branch_id,
                    self.current_user_id,
                    &changes,
                    false,
                )
                .map_err(|e| {
                    log_fatal(format!("create_descendants_commits: {:?}", e));
                });

                futures.push(f);
            }

            // change ancestors in batches of 25 parallel requests
            let _ = futures::future::join_all(futures).await;
        }

        Ok(())
    }
}
