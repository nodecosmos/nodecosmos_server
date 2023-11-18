mod recovery;
mod reorder_data;
mod validator;

use crate::errors::NodecosmosError;
use crate::models::node::{Node, UpdateOrderNode};
use crate::models::node_descendant::NodeDescendant;
use crate::services::resource_locker::ResourceLocker;
use crate::utils::logger::log_fatal;
use charybdis::batch::CharybdisModelBatch;
use charybdis::operations::{execute, Update};
use scylla::CachingSession;

use crate::api::data::RequestData;
use crate::api::types::{ActionObject, ActionTypes};
use crate::api::ReorderParams;
use crate::models::node::reorder::reorder_data::ReorderData;
use crate::models::node::reorder::validator::ReorderValidator;
pub(crate) use recovery::Recovery;

impl Node {
    pub async fn reorder(&self, ext: &RequestData, params: ReorderParams) -> Result<(), NodecosmosError> {
        let db_session = &ext.app.db_session;
        let resource_locker = &ext.app.resource_locker;

        resource_locker.check_node_lock(&self).await?;
        resource_locker
            .check_node_action_lock(&self, ActionTypes::Reorder(ActionObject::Node))
            .await?;

        let reorder_data = ReorderData::from_params(&params, &db_session).await?;
        let mut reorder = Reorder::new(&ext.app.db_session, reorder_data).await?;

        reorder.run(&ext.app.resource_locker).await?;

        Ok(())
    }
}

pub struct Reorder<'a> {
    pub db_session: &'a CachingSession,
    pub reorder_data: ReorderData,
}

const RESOURCE_LOCKER_TTL: usize = 1000 * 60 * 60; // 1 hour

impl<'a> Reorder<'a> {
    pub async fn new(db_session: &'a CachingSession, data: ReorderData) -> Result<Reorder<'a>, NodecosmosError> {
        Ok(Self {
            db_session,
            reorder_data: data,
        })
    }

    pub async fn run(&mut self, locker: &ResourceLocker) -> Result<(), NodecosmosError> {
        ReorderValidator::new(&self.reorder_data).validate()?;

        locker
            .lock_resource(&self.reorder_data.tree_root.id.to_string(), RESOURCE_LOCKER_TTL)
            .await?;

        let res = self.execute_reorder().await;

        return match res {
            Ok(_) => {
                locker
                    .unlock_resource(&self.reorder_data.tree_root.id.to_string())
                    .await?;
                Ok(())
            }
            Err(err) => {
                log_fatal(format!(
                    "Reorder failed for tree_root node: {}\n! ERROR: {:?}",
                    self.reorder_data.tree_root.id, err
                ));

                Recovery::new(&self.reorder_data, &self.db_session, locker)
                    .recover()
                    .await?;

                return Err(err);
            }
        };
    }

    async fn execute_reorder(&mut self) -> Result<(), NodecosmosError> {
        self.update_node_order().await?;
        self.remove_node_from_old_ancestors().await?;
        self.add_node_to_new_ancestors().await?;

        if self.reorder_data.is_parent_changed() {
            self.remove_old_ancestors_from_node().await?;
            self.remove_old_ancestors_from_descendants().await?;
            self.remove_node_descendants_from_old_ancestors().await?;

            self.add_new_ancestors_to_node().await?;
            self.add_new_ancestors_to_descendants().await?;
            self.add_node_descendants_to_new_ancestors().await?;
        }

        Ok(())
    }

    async fn update_node_order(&mut self) -> Result<(), NodecosmosError> {
        let update_order_node = UpdateOrderNode {
            id: self.reorder_data.node.id,
            parent_id: Some(self.reorder_data.new_parent.id),
            order_index: self.reorder_data.new_order_index,
        };

        update_order_node.update(&self.db_session).await?;

        println!("Updated node order: {:?}", update_order_node.id);

        Ok(())
    }

    async fn remove_node_from_old_ancestors(&mut self) -> Result<(), NodecosmosError> {
        let mut descendants_to_delete = vec![];

        for ancestor_id in self.reorder_data.old_node_ancestor_ids.clone() {
            let descendant = NodeDescendant {
                root_id: self.reorder_data.tree_root.id,
                node_id: ancestor_id,
                id: self.reorder_data.node.id,
                order_index: self.reorder_data.old_order_index,
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

        for ancestor_id in self.reorder_data.old_node_ancestor_ids.clone() {
            for descendant in &self.reorder_data.descendants {
                let descendant = NodeDescendant {
                    root_id: self.reorder_data.tree_root.id,
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
                log_fatal(format!("remove_node_descendants_from_old_ancestors: {:?}", err));
                return err;
            })?;

        Ok(())
    }

    async fn add_node_to_new_ancestors(&mut self) -> Result<(), NodecosmosError> {
        let mut descendants_to_add = vec![];

        for ancestor_id in self.reorder_data.new_node_ancestor_ids.clone() {
            let descendant = NodeDescendant {
                root_id: self.reorder_data.tree_root.id,
                node_id: ancestor_id,
                id: self.reorder_data.node.id,
                order_index: self.reorder_data.new_order_index,
                title: self.reorder_data.node.title.clone(),
                parent_id: self.reorder_data.new_parent.id,
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

        for ancestor_id in self.reorder_data.new_node_ancestor_ids.clone() {
            for descendant in self.reorder_data.descendants.clone() {
                let descendant = NodeDescendant {
                    root_id: self.reorder_data.tree_root.id,
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
            (
                self.reorder_data.old_node_ancestor_ids.clone(),
                self.reorder_data.node.id,
            ),
        )
        .await?;

        Ok(())
    }

    async fn remove_old_ancestors_from_descendants(&mut self) -> Result<(), NodecosmosError> {
        for descendant_chunk in self.reorder_data.descendant_ids.chunks(100) {
            let mut batch = CharybdisModelBatch::new();

            for descendant_id in descendant_chunk {
                batch.append_statement(
                    Node::PULL_FROM_ANCESTOR_IDS_QUERY,
                    (self.reorder_data.old_node_ancestor_ids.clone(), descendant_id),
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
            (
                self.reorder_data.new_node_ancestor_ids.clone(),
                self.reorder_data.node.id,
            ),
        )
        .await?;

        Ok(())
    }

    async fn add_new_ancestors_to_descendants(&mut self) -> Result<(), NodecosmosError> {
        for descendant_chunk in self.reorder_data.descendant_ids.chunks(100) {
            let mut batch = CharybdisModelBatch::new();

            for descendant_id in descendant_chunk {
                batch.append_statement(
                    Node::PUSH_TO_ANCESTOR_IDS_QUERY,
                    (self.reorder_data.new_node_ancestor_ids.clone(), descendant_id),
                )?;
            }

            batch.execute(&self.db_session).await?;
        }

        Ok(())
    }
}
