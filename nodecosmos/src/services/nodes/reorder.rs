use crate::errors::NodecosmosError;
use crate::models::helpers::clone_ref::ClonedRef;
use crate::models::node::{
    find_reorder_node_query, find_update_descendant_ids_query, Node, ReorderNode, UpdateAncestors, UpdateChildIds,
    UpdateDescendantIds, UpdateParent,
};
use crate::services::resource_locker::ResourceLocker;
use actix_web::web::Data;
use charybdis::{execute, CharybdisModelBatch, Deserialize, Find, Update, Uuid};
use scylla::CachingSession;
use std::collections::HashSet;

#[derive(Deserialize)]
pub struct ReorderParams {
    #[serde(rename = "rootId")]
    pub root_id: Uuid,

    #[serde(rename = "id")]
    pub id: Uuid,

    #[serde(rename = "newParentId")]
    pub new_parent_id: Uuid,

    #[serde(rename = "newSiblingIndex")]
    pub new_sibling_index: usize,
}

pub struct Reorderer {
    pub params: ReorderParams,
    pub root: ReorderNode,
    pub node: Node,
    pub new_parent: ReorderNode,
    pub new_node_ancestor_ids: Option<Vec<Uuid>>,
    pub db_session: Data<CachingSession>,
}

const RESOURCE_LOCKER_TTL: usize = 100000; // 100 seconds

impl Reorderer {
    pub async fn new(db_session: Data<CachingSession>, params: ReorderParams) -> Result<Self, NodecosmosError> {
        let root = ReorderNode {
            root_id: params.root_id,
            id: params.root_id,
            ..Default::default()
        }
        .find_by_primary_key(&db_session)
        .await?;

        let node = Node {
            root_id: params.root_id,
            id: params.id,
            ..Default::default()
        }
        .find_by_primary_key(&db_session)
        .await?;

        let new_parent = ReorderNode {
            root_id: params.root_id,
            id: params.new_parent_id,
            ..Default::default()
        }
        .find_by_primary_key(&db_session)
        .await?;

        Ok(Self {
            params,
            root,
            node,
            new_parent,
            new_node_ancestor_ids: None,
            db_session,
        })
    }

    pub async fn reorder(&mut self, resource_locker: &ResourceLocker) -> Result<(), NodecosmosError> {
        resource_locker
            .lock_resource(&self.root.id.to_string(), RESOURCE_LOCKER_TTL)
            .await?;

        self.add_node_to_parent().await?;

        // I initially used batch statements for all the queries, but I ran into a
        // problem where the batch statements were not executed in the order they were
        // added to the batch. This caused the reorder to fail.

        if self.is_parent_changed() {
            let descendant_ids = self.root.descendant_ids.clone().unwrap_or_default();

            let res = self.exec_reorder().await;

            return match res {
                Ok(_) => {
                    resource_locker.unlock_resource(&self.root.id.to_string()).await?;
                    Ok(())
                }
                Err(e) => {
                    println!("error in exec_reorder: {:?}", e);

                    self.root.descendant_ids = Some(descendant_ids);
                    self.root.update(&self.db_session).await?; // Prevent orphaned nodes

                    resource_locker.unlock_resource(&self.root.id.to_string()).await?;

                    return Err(e);
                }
            };
        }

        resource_locker.unlock_resource(&self.root.id.to_string()).await?;

        Ok(())
    }

    pub async fn exec_reorder(&mut self) -> Result<(), NodecosmosError> {
        let now = std::time::Instant::now();
        self.remove_from_old_parent().await?;
        println!("elapsed for remove_from_old_parent: {:?}", now.elapsed());

        let now = std::time::Instant::now();
        self.remove_node_from_old_ancestors().await?;
        println!("elapsed for remove_node_from_old_ancestors: {:?}", now.elapsed());

        let now = std::time::Instant::now();
        self.remove_node_descendants_from_old_ancestors().await?;
        println!(
            "elapsed for remove_node_descendants_from_old_ancestors: {:?}",
            now.elapsed()
        );

        self.build_new_ancestor_ids();

        let now = std::time::Instant::now();
        self.add_new_parent().await?;
        println!("elapsed for add_new_parent: {:?}", now.elapsed());

        let now = std::time::Instant::now();
        self.add_node_to_ancestor_descendants_and_op().await?;
        println!(
            "elapsed for add_node_to_ancestor_descendants_and_op: {:?}",
            now.elapsed()
        );

        let now = std::time::Instant::now();
        self.add_new_ancestors_to_descendants_and_op().await?;
        println!(
            "elapsed for add_new_ancestors_to_descendants_and_op: {:?}",
            now.elapsed()
        );

        Ok(())
    }

    fn is_parent_changed(&self) -> bool {
        if let Some(current_parent_id) = self.node.parent_id {
            return current_parent_id != self.params.new_parent_id;
        }

        false
    }

    fn build_new_ancestor_ids(&mut self) {
        let new_parent_ancestors = self.new_parent.ancestor_ids.cloned_ref();
        let mut new_ancestors = Vec::with_capacity(new_parent_ancestors.len() + 1);

        new_ancestors.extend(new_parent_ancestors);
        new_ancestors.push(self.new_parent.id);

        self.new_node_ancestor_ids = Some(new_ancestors);
    }

    async fn remove_from_old_parent(&mut self) -> Result<(), NodecosmosError> {
        if let Some(parent_id) = &self.node.parent_id {
            let query = Node::PULL_FROM_CHILD_IDS_QUERY;
            execute(&self.db_session, query, (self.node.id, self.node.root_id, parent_id)).await?;
        }

        Ok(())
    }

    async fn remove_node_from_old_ancestors(&mut self) -> Result<(), NodecosmosError> {
        let ancestor_ids = self.node.ancestor_ids.cloned_ref();
        let mut batch = CharybdisModelBatch::new();

        for ancestor_id in ancestor_ids {
            let query = Node::PULL_FROM_DESCENDANT_IDS_QUERY;
            batch.append_statement(query, (self.node.id, self.node.root_id, ancestor_id))?;
        }

        batch.execute(&self.db_session).await?;
        Ok(())
    }

    async fn remove_node_descendants_from_old_ancestors(&mut self) -> Result<(), NodecosmosError> {
        let ancestor_ids = self.node.ancestor_ids.cloned_ref();
        let node_descendant_ids = self.node.descendant_ids.cloned_ref();

        let to_remove_ids_hash: HashSet<_> = node_descendant_ids.into_iter().collect();
        let ancestor_ids_chunks = ancestor_ids.chunks(100);

        let mut batch = CharybdisModelBatch::new();

        for ancestor_ids in ancestor_ids_chunks.clone() {
            let ancestor_query = find_update_descendant_ids_query!("root_id = ? AND id IN ?");
            let ancestors =
                UpdateDescendantIds::find(&self.db_session, ancestor_query, (self.node.root_id, ancestor_ids)).await?;

            for ancestor in ancestors.flatten() {
                let mut new_descendant_ids = ancestor.descendant_ids.unwrap_or_default();
                new_descendant_ids.retain(|item| !to_remove_ids_hash.contains(item));

                let update_descendant_ids = UpdateDescendantIds {
                    root_id: self.node.root_id,
                    id: ancestor.id,
                    descendant_ids: Some(new_descendant_ids),
                };

                batch.append_update(update_descendant_ids)?;
            }
        }

        batch.execute(&self.db_session).await?;

        Ok(())
    }

    async fn add_new_parent(&mut self) -> Result<(), NodecosmosError> {
        let update_parent_node = UpdateParent {
            root_id: self.node.root_id,
            id: self.node.id,
            parent_id: Some(self.params.new_parent_id),
        };

        update_parent_node.update(&self.db_session).await?;

        Ok(())
    }

    async fn add_node_to_parent(&mut self) -> Result<(), NodecosmosError> {
        let mut new_parent_child_ids = self.new_parent.child_ids.cloned_ref();

        if !self.is_parent_changed() {
            new_parent_child_ids.retain(|id| id != &self.node.id);
        }

        new_parent_child_ids.insert(self.params.new_sibling_index, self.node.id);

        let update_child_ids_node = UpdateChildIds {
            root_id: self.node.root_id,
            id: self.params.new_parent_id,
            child_ids: Some(new_parent_child_ids),
        };

        update_child_ids_node.update(&self.db_session).await?;

        Ok(())
    }

    async fn add_node_to_ancestor_descendants_and_op(&mut self) -> Result<(), NodecosmosError> {
        let mut batch = CharybdisModelBatch::new();

        let new_node_ancestor_ids = self.new_node_ancestor_ids.cloned_ref();

        for ancestor_id in &new_node_ancestor_ids {
            let query = Node::PUSH_TO_DESCENDANT_IDS_QUERY;
            batch.append_statement(query, (self.node.id, self.node.root_id, *ancestor_id))?;
        }

        let update_ancestor_node = UpdateAncestors {
            root_id: self.node.root_id,
            id: self.node.id,
            ancestor_ids: Some(new_node_ancestor_ids),
        };

        batch.append_update(update_ancestor_node)?;

        batch.execute(&self.db_session).await?;

        Ok(())
    }

    async fn add_new_ancestors_to_descendants_and_op(&mut self) -> Result<(), NodecosmosError> {
        let descendant_ids = self.node.descendant_ids.clone().unwrap_or_default();

        let new_node_ancestor_ids = self.new_node_ancestor_ids.cloned_ref();

        let descendants_q = find_reorder_node_query!("root_id = ? AND id IN ?");
        let descendant_ids_chunks = descendant_ids.chunks(100);

        let mut descendants = Vec::with_capacity(descendant_ids.len());

        // get all descendants in batches
        for descendant_ids in descendant_ids_chunks {
            let descendants_batch =
                ReorderNode::find(&self.db_session, descendants_q, (self.root.id, descendant_ids)).await?;

            descendants.extend(descendants_batch.flatten());
        }

        for descendant in descendants {
            // TODO: figure out why it doesn't work if batch is created outside of the loop
            let mut batch = CharybdisModelBatch::new();

            // add descendant to new ancestors
            for ancestor_id in &new_node_ancestor_ids {
                let query = Node::PUSH_TO_DESCENDANT_IDS_QUERY;

                batch.append_statement(query, (descendant.id, self.node.root_id, *ancestor_id))?;
            }

            let current_ancestor_ids = descendant.ancestor_ids.cloned_ref();

            // current reordered node + 1 index
            let split_index = current_ancestor_ids
                .iter()
                .position(|id| id == &self.node.id)
                .unwrap_or_default();

            // take all existing ancestors bellow the reordered_node
            let preserved_ancestor_ids = current_ancestor_ids
                .iter()
                .skip(split_index)
                .cloned()
                .collect::<Vec<Uuid>>();

            // append new ancestors to preserved ancestors
            let mut new_complete_ancestor_ids = new_node_ancestor_ids.clone();
            new_complete_ancestor_ids.extend(preserved_ancestor_ids);

            let update_ancestors_node = UpdateAncestors {
                root_id: self.node.root_id,
                id: descendant.id,
                ancestor_ids: Some(new_complete_ancestor_ids),
            };

            batch.append_update(update_ancestors_node)?;
            batch.execute(&self.db_session).await?;
        }

        Ok(())
    }
}
