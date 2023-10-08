mod recovery;
mod reorder_data;
mod validator;

pub use recovery::recover_reorder_failures;

use crate::errors::NodecosmosError;
use crate::models::helpers::clone_ref::ClonedRef;
use crate::models::helpers::Pluckable;
use crate::models::node::{Node, ReorderNode, UpdateNodeOrder};
use crate::models::node_descendant::NodeDescendant;
use crate::services::logger::log_fatal;
use crate::services::nodes::reorder::recovery::recover_from_root;
use crate::services::nodes::reorder::reorder_data::{
    build_new_ancestor_ids, build_new_index, find_reorder_data,
};
use crate::services::nodes::reorder::validator::ReorderValidator;
use crate::services::resource_locker::ResourceLocker;
use charybdis::{execute, CharybdisModelBatch, Update, Uuid};
use scylla::CachingSession;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct ReorderParams {
    #[serde(rename = "nodeId")]
    pub node_id: Uuid,

    #[serde(rename = "newParentId")]
    pub new_parent_id: Uuid,

    #[serde(rename = "newUpperSiblingId")]
    pub new_upper_sibling_id: Option<Uuid>,

    #[serde(rename = "newBottomSiblingId")]
    pub new_bottom_sibling_id: Option<Uuid>,
}

pub struct Reorderer<'a> {
    pub params: ReorderParams,
    pub db_session: &'a CachingSession,
    pub root: ReorderNode,
    pub root_descendants: Vec<NodeDescendant>,
    pub node: Node,
    pub descendant_ids: Vec<Uuid>,
    pub descendants: Vec<NodeDescendant>,
    pub new_parent: ReorderNode,
    pub old_node_ancestor_ids: Vec<Uuid>,
    pub new_node_ancestor_ids: Vec<Uuid>,
    pub new_upper_sibling: Option<ReorderNode>,
    pub new_bottom_sibling: Option<ReorderNode>,
    pub old_order_index: f64,
    pub new_order_index: f64,
}

const RESOURCE_LOCKER_TTL: usize = 100000; // 100 seconds

impl<'a> Reorderer<'a> {
    pub async fn new(
        db_session: &'a CachingSession,
        params: ReorderParams,
    ) -> Result<Reorderer<'a>, NodecosmosError> {
        let reorder_data = find_reorder_data(&db_session, &params).await?;

        let root = reorder_data.root;
        let root_descendants = reorder_data.root_descendants;

        let node = reorder_data.node;
        let descendant_ids = reorder_data.descendants.pluck_id();
        let descendants = reorder_data.descendants;

        let new_parent = reorder_data.new_parent;

        let new_upper_sibling = reorder_data.new_upper_sibling;
        let new_bottom_sibling = reorder_data.new_bottom_sibling;

        let old_node_ancestor_ids = node.ancestor_ids.cloned_ref();
        let new_node_ancestor_ids = build_new_ancestor_ids(&new_parent);

        let old_order_index = node.order_index.unwrap_or_default();
        let new_order_index = build_new_index(&new_upper_sibling, &new_bottom_sibling).await?;

        Ok(Self {
            params,
            db_session,
            root,
            root_descendants,
            node,
            descendant_ids,
            descendants,
            new_parent,
            old_node_ancestor_ids,
            new_node_ancestor_ids,
            new_upper_sibling,
            new_bottom_sibling,
            old_order_index,
            new_order_index,
        })
    }

    pub async fn reorder(&mut self, locker: &ResourceLocker) -> Result<(), NodecosmosError> {
        ReorderValidator::new(self).validate()?;

        locker
            .lock(&self.root.id.to_string(), RESOURCE_LOCKER_TTL)
            .await?;

        let res = self.execute_reorder().await;

        return match res {
            Ok(_) => {
                locker.unlock(&self.root.id.to_string()).await?;
                Ok(())
            }
            Err(err) => {
                log_fatal(format!("exec_reorder: {:?}", err));

                recover_from_root(&self.root, &self.root_descendants, &self.db_session).await;

                locker.unlock(&self.root.id.to_string()).await?;
                return Err(err);
            }
        };
    }

    async fn execute_reorder(&mut self) -> Result<(), NodecosmosError> {
        self.update_node_order().await?;
        self.remove_node_from_old_ancestors().await?;
        self.add_node_to_new_ancestors().await?;

        if self.is_parent_changed() {
            self.remove_old_ancestors_from_node().await?;
            self.remove_old_ancestors_from_descendants().await?;
            self.remove_node_descendants_from_old_ancestors().await?;

            self.add_new_ancestors_to_node().await?;
            self.add_new_ancestors_to_descendants().await?;
            self.add_node_descendants_to_new_ancestors().await?;
        }

        Ok(())
    }

    fn is_parent_changed(&self) -> bool {
        if let Some(current_parent_id) = self.node.parent_id {
            return current_parent_id != self.params.new_parent_id;
        }

        false
    }

    async fn update_node_order(&mut self) -> Result<(), NodecosmosError> {
        let update_order_node = UpdateNodeOrder {
            id: self.node.id,
            parent_id: Some(self.params.new_parent_id),
            order_index: Some(self.new_order_index),
        };

        update_order_node.update(&self.db_session).await?;

        Ok(())
    }

    async fn remove_node_from_old_ancestors(&mut self) -> Result<(), NodecosmosError> {
        let mut descendants_to_delete = vec![];

        for ancestor_id in self.old_node_ancestor_ids.clone() {
            let descendant = NodeDescendant {
                root_id: self.root.id,
                node_id: ancestor_id,
                id: self.node.id,
                order_index: self.old_order_index,
                ..Default::default()
            };

            descendants_to_delete.push(descendant);
        }

        CharybdisModelBatch::chunked_delete(&self.db_session, &descendants_to_delete, 100)
            .await
            .map_err(|err| {
                log_fatal(format!("remove_node_from_old_ancestors: {:?}", err));
                return err;
            })?;

        Ok(())
    }

    async fn remove_node_descendants_from_old_ancestors(&mut self) -> Result<(), NodecosmosError> {
        let mut descendants_to_delete = vec![];

        for ancestor_id in self.old_node_ancestor_ids.clone() {
            for descendant in self.descendants.clone() {
                let descendant = NodeDescendant {
                    root_id: self.root.id,
                    node_id: ancestor_id,
                    id: descendant.id,
                    order_index: descendant.order_index,
                    ..Default::default()
                };

                descendants_to_delete.push(descendant);
            }
        }

        CharybdisModelBatch::chunked_delete(&self.db_session, &descendants_to_delete, 100)
            .await
            .map_err(|err| {
                log_fatal(format!(
                    "remove_node_descendants_from_old_ancestors: {:?}",
                    err
                ));
                return err;
            })?;

        Ok(())
    }

    async fn add_node_to_new_ancestors(&mut self) -> Result<(), NodecosmosError> {
        let mut descendants_to_add = vec![];

        for ancestor_id in self.new_node_ancestor_ids.clone() {
            let descendant = NodeDescendant {
                root_id: self.root.id,
                node_id: ancestor_id,
                id: self.node.id,
                order_index: self.new_order_index,
                title: self.node.title.clone(),
                parent_id: Some(self.params.new_parent_id),
            };

            descendants_to_add.push(descendant);
        }

        CharybdisModelBatch::chunked_insert(&self.db_session, &descendants_to_add, 100)
            .await
            .map_err(|err| {
                log_fatal(format!("add_node_to_new_ancestors: {:?}", err));
                return err;
            })?;

        Ok(())
    }

    async fn add_node_descendants_to_new_ancestors(&mut self) -> Result<(), NodecosmosError> {
        let mut descendants_to_add = vec![];

        for ancestor_id in self.new_node_ancestor_ids.clone() {
            for descendant in self.descendants.clone() {
                let descendant = NodeDescendant {
                    root_id: self.root.id,
                    node_id: ancestor_id,
                    id: descendant.id,
                    order_index: descendant.order_index,
                    title: descendant.title.clone(),
                    parent_id: descendant.parent_id,
                };

                descendants_to_add.push(descendant);
            }
        }

        CharybdisModelBatch::chunked_insert(&self.db_session, &descendants_to_add, 100)
            .await
            .map_err(|err| {
                log_fatal(format!("add_node_to_new_ancestors: {:?}", err));
                return err;
            })?;

        Ok(())
    }

    async fn remove_old_ancestors_from_node(&mut self) -> Result<(), NodecosmosError> {
        execute(
            self.db_session,
            Node::PULL_FROM_ANCESTOR_IDS_QUERY,
            (self.old_node_ancestor_ids.clone(), self.node.id),
        )
        .await?;

        Ok(())
    }

    async fn remove_old_ancestors_from_descendants(&mut self) -> Result<(), NodecosmosError> {
        for descendant_chunk in self.descendant_ids.chunks(100) {
            let mut batch = CharybdisModelBatch::new();

            for descendant_id in descendant_chunk {
                batch.append_statement(
                    Node::PULL_FROM_ANCESTOR_IDS_QUERY,
                    (self.old_node_ancestor_ids.clone(), descendant_id),
                )?;
            }

            batch.execute(&self.db_session).await?;
        }

        Ok(())
    }

    async fn add_new_ancestors_to_node(&mut self) -> Result<(), NodecosmosError> {
        execute(
            self.db_session,
            Node::PUSH_TO_ANCESTOR_IDS_QUERY,
            (self.new_node_ancestor_ids.clone(), self.node.id),
        )
        .await?;

        Ok(())
    }

    async fn add_new_ancestors_to_descendants(&mut self) -> Result<(), NodecosmosError> {
        for descendant_chunk in self.descendant_ids.chunks(100) {
            let mut batch = CharybdisModelBatch::new();

            for descendant_id in descendant_chunk {
                batch.append_statement(
                    Node::PUSH_TO_ANCESTOR_IDS_QUERY,
                    (self.new_node_ancestor_ids.clone(), descendant_id),
                )?;
            }

            batch.execute(&self.db_session).await?;
        }

        Ok(())
    }
}
