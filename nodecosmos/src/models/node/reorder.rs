use charybdis::batch::ModelBatch;
use charybdis::operations::{execute, Update};
use charybdis::options::Consistency;
use charybdis::types::{Double, Uuid};
use log::error;
use scylla::CachingSession;
use serde::Deserialize;

use nodecosmos_macros::Branchable;
pub use recovery::Recovery;

use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::branch::update::BranchUpdate;
use crate::models::branch::Branch;
use crate::models::node::reorder::data::ReorderData;
use crate::models::node::reorder::validator::ReorderValidator;
use crate::models::node::{Node, UpdateOrderNode};
use crate::models::node_descendant::NodeDescendant;
use crate::models::traits::Branchable;
use crate::models::udts::BranchReorderData;

pub mod data;
mod recovery;
mod validator;

// TODO: update to use SAGA similar to `BranchMerge` so we avoid reading whole tree for recovery
#[derive(Deserialize, Branchable)]
#[serde(rename_all = "camelCase")]
pub struct ReorderParams {
    #[branch(original_id)]
    pub root_id: Uuid,
    pub branch_id: Uuid,
    pub id: Uuid,
    pub new_parent_id: Uuid,
    pub new_upper_sibling_id: Option<Uuid>,
    pub new_lower_sibling_id: Option<Uuid>,
    // we provide order_index only in context of merge recovery
    pub new_order_index: Option<Double>,
}

pub struct Reorder<'a> {
    pub db_session: &'a CachingSession,
    pub request_data: &'a RequestData,
    pub reorder_data: &'a ReorderData,
}

impl<'a> Reorder<'a> {
    pub fn new(request_data: &'a RequestData, data: &'a ReorderData) -> Reorder<'a> {
        Self {
            db_session: request_data.db_session(),
            request_data,
            reorder_data: data,
        }
    }

    pub async fn run(&mut self) -> Result<(), NodecosmosError> {
        let res = self.execute_reorder().await;

        if let Err(err) = res {
            error!(
                "Reorder failed for tree_root node: {}\n! ERROR: {:?}",
                self.reorder_data.tree_root.id, err
            );

            Recovery::new(
                &self.reorder_data,
                &self.db_session,
                self.request_data.resource_locker(),
            )
            .recover()
            .await?;

            return Err(err);
        }

        Ok(())
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

        self.update_branch().await?;

        Ok(())
    }

    async fn update_node_order(&mut self) -> Result<(), NodecosmosError> {
        let update_order_node = UpdateOrderNode {
            id: self.reorder_data.node.id,
            branch_id: self.reorder_data.branch_id,
            root_id: self.reorder_data.tree_root.id,
            parent_id: Some(self.reorder_data.new_parent.id),
            order_index: self.reorder_data.new_order_index,
        };

        update_order_node
            .update()
            .consistency(Consistency::EachQuorum)
            .execute(&self.db_session)
            .await?;

        Ok(())
    }

    async fn remove_node_from_old_ancestors(&mut self) -> Result<(), NodecosmosError> {
        let mut descendants_to_delete = vec![];

        for ancestor_id in self.reorder_data.old_ancestor_ids.clone() {
            let descendant = NodeDescendant {
                root_id: self.reorder_data.tree_root.id,
                branch_id: self.reorder_data.branch_id,
                node_id: ancestor_id,
                id: self.reorder_data.node.id,
                order_index: self.reorder_data.old_order_index,
                ..Default::default()
            };

            descendants_to_delete.push(descendant);
        }

        NodeDescendant::unlogged_delete_batch()
            .consistency(Consistency::EachQuorum)
            .chunked_delete(
                &self.db_session,
                &descendants_to_delete,
                crate::constants::MAX_PARALLEL_REQUESTS,
            )
            .await
            .map_err(|err| {
                error!("remove_node_from_removed_ancestors: {:?}", err);
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
                    branch_id: self.reorder_data.branch_id,
                    order_index: descendant.order_index,
                    ..Default::default()
                };

                descendants_to_delete.push(descendant);
            }
        }

        NodeDescendant::unlogged_delete_batch()
            .consistency(Consistency::EachQuorum)
            .chunked_delete(
                &self.db_session,
                &descendants_to_delete,
                crate::constants::MAX_PARALLEL_REQUESTS,
            )
            .await
            .map_err(|err| {
                error!("delete_node_descendants_from_removed_ancestors: {:?}", err);
                return err;
            })?;

        Ok(())
    }

    async fn add_node_to_new_ancestors(&mut self) -> Result<(), NodecosmosError> {
        let mut descendants_to_add = vec![];

        for ancestor_id in self.reorder_data.new_ancestor_ids.clone() {
            let descendant = NodeDescendant {
                root_id: self.reorder_data.tree_root.id,
                branch_id: self.reorder_data.branch_id,
                node_id: ancestor_id,
                id: self.reorder_data.node.id,
                order_index: self.reorder_data.new_order_index,
                title: self.reorder_data.node.title.clone(),
                parent_id: self.reorder_data.new_parent.id,
            };

            descendants_to_add.push(descendant);
        }

        NodeDescendant::unlogged_batch()
            .consistency(Consistency::EachQuorum)
            .chunked_insert(
                &self.db_session,
                &descendants_to_add,
                crate::constants::MAX_PARALLEL_REQUESTS,
            )
            .await
            .map_err(|err| {
                error!("add_node_to_new_ancestors: {:?}", err);
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
                    branch_id: self.reorder_data.branch_id,
                    node_id: ancestor_id,
                    id: descendant.id,
                    order_index: descendant.order_index,
                    title: descendant.title.clone(),
                    parent_id: descendant.parent_id,
                };

                descendants.push(descendant);
            }
        }

        NodeDescendant::unlogged_batch()
            .consistency(Consistency::EachQuorum)
            .chunked_insert(&self.db_session, &descendants, crate::constants::BATCH_CHUNK_SIZE)
            .await
            .map_err(|err| {
                error!("insert_node_descendants_to_added_ancestors: {:?}", err);
                return err;
            })?;

        Ok(())
    }

    async fn pull_removed_ancestors_from_node(&mut self) -> Result<(), NodecosmosError> {
        self.reorder_data
            .node
            .pull_ancestor_ids(&self.reorder_data.removed_ancestor_ids)
            .execute(&self.db_session)
            .await?;

        Ok(())
    }

    async fn pull_removed_ancestors_from_descendants(&mut self) -> Result<(), NodecosmosError> {
        let mut values = vec![];

        for descendant_id in &self.reorder_data.descendant_ids {
            let val = (
                self.reorder_data.removed_ancestor_ids.clone(),
                descendant_id,
                self.reorder_data.branch_id,
            );

            values.push(val);
        }

        Node::unlogged_statement_batch()
            .consistency(Consistency::EachQuorum)
            .chunked_statements(
                &self.db_session,
                Node::PULL_ANCESTOR_IDS_QUERY,
                values,
                crate::constants::MAX_PARALLEL_REQUESTS,
            )
            .await
            .map_err(|err| {
                error!("pull_removed_ancestors_from_descendants: {:?}", err);
                return err;
            })?;

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
        let mut values = vec![];

        for descendant_id in &self.reorder_data.descendant_ids {
            let val = (
                self.reorder_data.added_ancestor_ids.clone(),
                descendant_id,
                self.reorder_data.branch_id,
            );

            values.push(val);
        }

        Node::unlogged_statement_batch()
            .consistency(Consistency::EachQuorum)
            .chunked_statements(
                &self.db_session,
                Node::PUSH_ANCESTOR_IDS_QUERY,
                values,
                crate::constants::MAX_PARALLEL_REQUESTS,
            )
            .await
            .map_err(|err| {
                error!("push_added_ancestors_to_descendants: {:?}", err);
                return err;
            })?;

        Ok(())
    }

    async fn update_branch(&mut self) -> Result<(), NodecosmosError> {
        if self.reorder_data.is_branched() {
            Branch::update(
                self.request_data,
                self.reorder_data.branch_id,
                BranchUpdate::ReorderNode(BranchReorderData {
                    id: self.reorder_data.node.id,
                    new_parent_id: self.reorder_data.new_parent.id,
                    new_upper_sibling_id: self.reorder_data.new_upper_sibling_id,
                    new_lower_sibling_id: self.reorder_data.new_lower_sibling_id,
                    old_parent_id: self.reorder_data.old_parent_id.unwrap_or_default(),
                    old_order_index: self.reorder_data.old_order_index,
                }),
            )
            .await?;
        }

        Ok(())
    }
}

impl Node {
    pub async fn reorder(&self, data: &RequestData, params: ReorderParams) -> Result<(), NodecosmosError> {
        let reorder_data = ReorderData::from_params(&params, data).await?;

        ReorderValidator::new(&reorder_data).validate()?;
        Reorder::new(data, &reorder_data).run().await?;

        Ok(())
    }
}
