mod recovery;

use crate::errors::NodecosmosError;
use crate::models::helpers::clone_ref::ClonedRef;
use crate::models::node::{
    find_update_node_ancestor_ids_query, Node, ReorderNode, UpdateNodeAncestorIds, UpdateNodeOrder,
};
use crate::models::node_descendant::{DeleteNodeDescendant, NodeDescendant};
use crate::services::nodes::reorder::recovery::recover_root_node;
use crate::services::resource_locker::ResourceLocker;
use actix_web::web::Data;
use charybdis::{CharybdisModelBatch, Delete, Deserialize, Find, Insert, Update, Uuid};
use scylla::CachingSession;

#[derive(Deserialize)]
pub struct ReorderParams {
    #[serde(rename = "rootId")]
    pub root_id: Uuid,

    #[serde(rename = "id")]
    pub id: Uuid,

    #[serde(rename = "newParentId")]
    pub new_parent_id: Uuid,

    #[serde(rename = "newBottomSiblingId")]
    pub new_bottom_sibling_id: Option<Uuid>,
}

pub struct Reorderer {
    pub params: ReorderParams,
    pub root: ReorderNode,
    pub current_root_descendants: Vec<NodeDescendant>,
    pub node: Node,
    pub descendants: Vec<NodeDescendant>,
    pub descendant_ids: Vec<Uuid>,
    pub new_parent: ReorderNode,
    pub new_node_ancestor_ids: Option<Vec<Uuid>>,
    pub old_order_index: f64,
    pub new_order_index: f64,
    pub db_session: Data<CachingSession>,
}

const RESOURCE_LOCKER_TTL: usize = 100000; // 100 seconds
const ORDER_CORRECTION: f64 = 0.000000001;
const REORDER_DESCENDANTS_LIMIT: usize = 5000;

impl Reorderer {
    pub async fn new(
        db_session: Data<CachingSession>,
        params: ReorderParams,
    ) -> Result<Self, NodecosmosError> {
        let root = ReorderNode {
            id: params.root_id,
            ..Default::default()
        }
        .find_by_primary_key(&db_session)
        .await?;

        let current_root_descendants = NodeDescendant {
            root_id: params.root_id,
            ..Default::default()
        }
        .find_by_partition_key(&db_session)
        .await?;

        let node = Node {
            id: params.id,
            ..Default::default()
        }
        .find_by_primary_key(&db_session)
        .await?;

        let new_parent = ReorderNode {
            id: params.new_parent_id,
            ..Default::default()
        }
        .find_by_primary_key(&db_session)
        .await?;

        let descendants = NodeDescendant {
            root_id: params.id,
            ..Default::default()
        }
        .find_by_partition_key(&db_session)
        .await?;

        let descendant_ids = descendants
            .iter()
            .map(|descendant| descendant.id)
            .collect::<Vec<Uuid>>();

        let old_order_index = node.order_index.clone().unwrap_or_default();

        let mut new_order_index = 0f64;

        if let Some(new_bottom_sibling_id) = params.new_bottom_sibling_id {
            let new_bottom_sibling = UpdateNodeOrder {
                id: new_bottom_sibling_id,
                ..Default::default()
            }
            .find_by_primary_key(&db_session)
            .await?;
            let bottom_sibling_order_index = new_bottom_sibling.order_index.unwrap_or_default();

            if bottom_sibling_order_index == 0f64 {
                new_order_index = -0.1;
            } else {
                new_order_index = bottom_sibling_order_index - ORDER_CORRECTION;
            }
        }

        Ok(Self {
            params,
            root,
            current_root_descendants,
            node,
            descendants,
            descendant_ids,
            new_parent,
            new_node_ancestor_ids: None,
            old_order_index,
            new_order_index,
            db_session,
        })
    }

    pub async fn reorder(
        &mut self,
        resource_locker: &ResourceLocker,
    ) -> Result<(), NodecosmosError> {
        if let Some(is_root) = self.node.is_root {
            if is_root {
                return Err(NodecosmosError::Forbidden(
                    "Reorder is not allowed for root nodes".to_string(),
                ));
            }
        }

        if self.descendant_ids.contains(&self.params.new_parent_id)
            || self.node.id == self.params.new_parent_id
        {
            return Err(NodecosmosError::Forbidden(
                "Can not reorder within self".to_string(),
            ));
        }

        if self.is_parent_changed() && self.descendant_ids.len() > REORDER_DESCENDANTS_LIMIT {
            return Err(NodecosmosError::Forbidden(format!(
                "Can not reorder more than {} descendants",
                REORDER_DESCENDANTS_LIMIT
            )));
        }

        resource_locker
            .lock_resource(&self.root.id.to_string(), RESOURCE_LOCKER_TTL)
            .await?;

        let res = self.execute_reorder().await;

        return match res {
            Ok(_) => {
                resource_locker
                    .unlock_resource(&self.root.id.to_string())
                    .await?;
                Ok(())
            }
            Err(err) => {
                println!("FATAL error in exec_reorder: {:?}", err);

                if self.is_parent_changed() {
                    recover_root_node(&self.root, &self.current_root_descendants, &self.db_session)
                        .await;
                }

                resource_locker
                    .unlock_resource(&self.root.id.to_string())
                    .await?;

                return Err(err);
            }
        };
    }

    async fn execute_reorder(&mut self) -> Result<(), NodecosmosError> {
        self.update_node_order().await?;

        if self.is_parent_changed() {
            self.remove_node_from_old_ancestors().await?;
            self.build_new_ancestor_ids();
            self.add_new_ancestors().await?;
            self.add_node_to_new_ancestors().await?;
        }

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

    async fn update_node_order(&mut self) -> Result<(), NodecosmosError> {
        let update_order_node = UpdateNodeOrder {
            id: self.node.id,
            parent_id: Some(self.params.new_parent_id),
            order_index: Some(self.new_order_index),
        };

        update_order_node.update(&self.db_session).await?;

        if !self.is_parent_changed() {
            // just update node order_index for all ancestors
            for ancestor_id in self.node.ancestor_ids.cloned_ref() {
                let node_descendant = NodeDescendant {
                    root_id: ancestor_id,
                    id: self.node.id,
                    order_index: Some(self.old_order_index),
                    ..Default::default()
                };

                node_descendant.delete(&self.db_session).await?;

                let node_descendant = NodeDescendant {
                    root_id: ancestor_id,
                    id: self.node.id,
                    order_index: Some(self.new_order_index),
                    title: self.node.title.clone(),
                    parent_id: Some(self.params.new_parent_id),
                };

                node_descendant.insert(&self.db_session).await?;
            }
        }

        Ok(())
    }

    /// Removes nodes and its descendants from old ancestors
    async fn remove_node_from_old_ancestors(&mut self) -> Result<(), NodecosmosError> {
        let old_ancestor_ids = self.node.ancestor_ids.cloned_ref();

        for ancestor_id in old_ancestor_ids {
            // delete node from ancestors' descendants
            let descendant = DeleteNodeDescendant {
                root_id: ancestor_id,
                id: self.node.id,
                order_index: Some(self.old_order_index),
            };

            descendant.delete(&self.db_session).await?;

            // delete node's descendants from ancestors' descendants
            let descendant_chunks = self.descendants.chunks(100);
            for descendants in descendant_chunks {
                let mut batch = CharybdisModelBatch::new();

                for descendant in descendants {
                    let descendant = DeleteNodeDescendant {
                        root_id: ancestor_id,
                        id: descendant.id,
                        order_index: descendant.order_index,
                    };

                    batch.append_delete(&descendant)?;
                }

                batch.execute(&self.db_session).await?;
            }
        }

        Ok(())
    }

    /// Adds new ancestors to nodes and its descendants
    async fn add_new_ancestors(&mut self) -> Result<(), NodecosmosError> {
        let new_node_ancestor_ids = self.new_node_ancestor_ids.cloned_ref();
        let update_ancestors_node = UpdateNodeAncestorIds {
            id: self.node.id,
            ancestor_ids: Some(new_node_ancestor_ids.clone()),
        };

        update_ancestors_node.update(&self.db_session).await?;

        let descendant_ids_chunks = self.descendant_ids.chunks(100);

        for descendant_ids in descendant_ids_chunks {
            let mut batch = CharybdisModelBatch::new();
            let update_ancestor_nodes = UpdateNodeAncestorIds::find(
                &self.db_session,
                find_update_node_ancestor_ids_query!("id IN ?"),
                (descendant_ids,),
            )
            .await?
            .flatten();

            for update_ancestor_node in update_ancestor_nodes {
                let current_ancestor_ids = update_ancestor_node.ancestor_ids.cloned_ref();

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

                let update_ancestors_node = UpdateNodeAncestorIds {
                    id: update_ancestor_node.id,
                    ancestor_ids: Some(new_complete_ancestor_ids),
                };

                batch.append_update(&update_ancestors_node)?;
            }

            batch.execute(&self.db_session).await?;
        }

        Ok(())
    }

    /// Adds nodes and its descendants to new ancestors
    async fn add_node_to_new_ancestors(&mut self) -> Result<(), NodecosmosError> {
        let mut descendant_ids_to_add = vec![self.node.id];
        descendant_ids_to_add.extend(self.descendant_ids.clone());

        if let Some(ancestor_ids) = self.new_node_ancestor_ids.clone() {
            for ancestor_id in ancestor_ids {
                let descendant = NodeDescendant {
                    root_id: ancestor_id,
                    id: self.node.id,
                    order_index: Some(self.new_order_index),
                    title: self.node.title.clone(),
                    parent_id: Some(self.params.new_parent_id),
                };

                descendant.insert(&self.db_session).await?;

                let descendant_chunks = self.descendants.chunks(100);

                for chunk in descendant_chunks {
                    let mut batch = CharybdisModelBatch::new();

                    for descendant in chunk {
                        let descendant = NodeDescendant {
                            root_id: ancestor_id,
                            id: descendant.id,
                            order_index: descendant.order_index,
                            title: descendant.title.clone(),
                            parent_id: descendant.parent_id,
                        };

                        batch.append_create(&descendant)?;
                    }

                    batch.execute(&self.db_session).await?;
                }
            }
        }

        Ok(())
    }
}
