mod recovery;
pub mod reorder_data;
mod validator;

use crate::api::data::RequestData;
use crate::api::types::{ActionObject, ActionTypes};
use crate::errors::NodecosmosError;
use crate::models::branch::branchable::Branchable;
use crate::models::node::reorder::reorder_data::ReorderData;
use crate::models::node::reorder::validator::ReorderValidator;
use crate::models::node::{Node, UpdateOrderNode};
use crate::models::node_commit::NodeCommit;
use crate::models::node_descendant::NodeDescendant;
use crate::services::resource_locker::ResourceLocker;
use crate::utils::logger::log_fatal;
use charybdis::batch::CharybdisModelBatch;
use charybdis::operations::{execute, Update};
use charybdis::types::Uuid;
pub(crate) use recovery::Recovery;
use scylla::CachingSession;
use serde::Deserialize;
use std::sync::Arc;

#[derive(Deserialize)]
pub struct ReorderParams {
    pub id: Uuid,

    #[serde(rename = "branchId")]
    pub branch_id: Uuid,

    #[serde(rename = "newParentId")]
    pub new_parent_id: Uuid,

    #[serde(rename = "newUpperSiblingId")]
    pub new_upper_sibling_id: Option<Uuid>,

    #[serde(rename = "newLowerSiblingId")]
    pub new_lower_sibling_id: Option<Uuid>,
}

impl Node {
    pub async fn reorder(&self, req_data: &RequestData, params: ReorderParams) -> Result<(), NodecosmosError> {
        req_data.resource_locker().validate_node_unlocked(&self, false).await?;
        req_data
            .resource_locker()
            .validate_action_unlocked(&self, ActionTypes::Reorder(ActionObject::Node), true)
            .await?;

        let reorder_data = ReorderData::from_params(&params, req_data.db_session()).await?;
        let reorder_data = Arc::new(reorder_data);

        let mut reorder = Reorder::new(req_data.db_session(), &reorder_data).await?;

        reorder.run(req_data.resource_locker()).await?;

        NodeCommit::handle_reorder(&req_data, &reorder_data).await?;

        Ok(())
    }
}

pub struct Reorder<'a> {
    pub db_session: &'a CachingSession,
    pub reorder_data: &'a ReorderData,
}

const RESOURCE_LOCKER_TTL: usize = 1000 * 60 * 60; // 1 hour

impl<'a> Reorder<'a> {
    pub async fn new(db_session: &'a CachingSession, data: &'a ReorderData) -> Result<Reorder<'a>, NodecosmosError> {
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

        if self.reorder_data.parent_changed() {
            self.pull_removed_ancestors_from_node().await?;
            self.pull_removed_ancestors_from_descendants().await?;
            self.delete_node_descendants_from_removed_ancestors().await?;

            self.push_added_ancestors_to_node().await?;
            self.push_added_ancestors_to_descendants().await?;
            self.insert_node_descendants_to_added_ancestors().await?;
        }

        Ok(())
    }

    async fn update_node_order(&mut self) -> Result<(), NodecosmosError> {
        let update_order_node = UpdateOrderNode {
            id: self.reorder_data.node.id,
            branch_id: self.reorder_data.branch_id,
            parent_id: Some(self.reorder_data.new_parent.id),
            order_index: self.reorder_data.new_order_index,
        };

        update_order_node.update(&self.db_session).await?;

        Ok(())
    }

    async fn remove_node_from_old_ancestors(&mut self) -> Result<(), NodecosmosError> {
        let mut descendants_to_delete = vec![];

        for ancestor_id in self.reorder_data.old_ancestor_ids.clone() {
            let descendant = NodeDescendant {
                root_id: self.reorder_data.tree_root.id,
                branch_id: self.reorder_data.branched_id(ancestor_id),
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
                log_fatal(format!("remove_node_from_removed_ancestors: {:?}", err));
                return err;
            })?;

        Ok(())
    }

    async fn delete_node_descendants_from_removed_ancestors(&mut self) -> Result<(), NodecosmosError> {
        let mut descendants_to_delete = vec![];

        for ancestor_id in self.reorder_data.removed_ancestor_ids.clone() {
            for descendant in &self.reorder_data.descendants {
                let descendant = NodeDescendant {
                    root_id: self.reorder_data.tree_root.id,
                    node_id: ancestor_id,
                    id: descendant.id,
                    branch_id: self.reorder_data.branched_id(ancestor_id),
                    order_index: descendant.order_index,
                    ..Default::default()
                };

                descendants_to_delete.push(descendant);
            }
        }

        CharybdisModelBatch::chunked_delete(&self.db_session, &descendants_to_delete, 100)
            .await
            .map_err(|err| {
                log_fatal(format!("delete_node_descendants_from_removed_ancestors: {:?}", err));
                return err;
            })?;

        Ok(())
    }

    async fn add_node_to_new_ancestors(&mut self) -> Result<(), NodecosmosError> {
        let mut descendants_to_add = vec![];

        for ancestor_id in self.reorder_data.new_ancestor_ids.clone() {
            let descendant = NodeDescendant {
                root_id: self.reorder_data.tree_root.id,
                branch_id: self.reorder_data.branched_id(ancestor_id),
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

    async fn insert_node_descendants_to_added_ancestors(&mut self) -> Result<(), NodecosmosError> {
        let mut descendants = vec![];

        for ancestor_id in self.reorder_data.added_ancestor_ids.clone() {
            for descendant in self.reorder_data.descendants.clone() {
                let descendant = NodeDescendant {
                    root_id: self.reorder_data.tree_root.id,
                    branch_id: self.reorder_data.branched_id(ancestor_id),
                    node_id: ancestor_id,
                    id: descendant.id,
                    order_index: descendant.order_index,
                    title: descendant.title.clone(),
                    parent_id: descendant.parent_id,
                };

                descendants.push(descendant);
            }
        }

        CharybdisModelBatch::chunked_insert(&self.db_session, &descendants, 100)
            .await
            .map_err(|err| {
                log_fatal(format!("add_node_to_new_ancestors: {:?}", err));
                return err;
            })?;

        Ok(())
    }

    async fn pull_removed_ancestors_from_node(&mut self) -> Result<(), NodecosmosError> {
        execute(
            self.db_session,
            Node::PULL_ANCESTOR_IDS_QUERY,
            (
                self.reorder_data.removed_ancestor_ids.clone(),
                self.reorder_data.node.id,
                self.reorder_data.node.branch_id,
            ),
        )
        .await?;

        Ok(())
    }

    async fn pull_removed_ancestors_from_descendants(&mut self) -> Result<(), NodecosmosError> {
        for descendant_chunk in self.reorder_data.descendant_ids.chunks(100) {
            let mut batch = CharybdisModelBatch::new();

            for descendant_id in descendant_chunk {
                batch.append_statement(
                    Node::PULL_ANCESTOR_IDS_QUERY,
                    (
                        self.reorder_data.removed_ancestor_ids.clone(),
                        descendant_id,
                        self.reorder_data.branched_id(*descendant_id),
                    ),
                )?;
            }

            batch.execute(&self.db_session).await?;
        }

        Ok(())
    }

    async fn push_added_ancestors_to_node(&mut self) -> Result<(), NodecosmosError> {
        execute(
            self.db_session,
            Node::PUSH_ANCESTOR_IDS_QUERY,
            (
                self.reorder_data.added_ancestor_ids.clone(),
                self.reorder_data.node.id,
                self.reorder_data.branch_id,
            ),
        )
        .await?;

        Ok(())
    }

    async fn push_added_ancestors_to_descendants(&mut self) -> Result<(), NodecosmosError> {
        for descendant_chunk in self.reorder_data.descendant_ids.chunks(100) {
            let mut batch = CharybdisModelBatch::new();

            for descendant_id in descendant_chunk {
                batch.append_statement(
                    Node::PUSH_ANCESTOR_IDS_QUERY,
                    (
                        self.reorder_data.added_ancestor_ids.clone(),
                        descendant_id,
                        self.reorder_data.branched_id(*descendant_id),
                    ),
                )?;
            }

            batch.execute(&self.db_session).await?;
        }

        Ok(())
    }
}
