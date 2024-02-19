use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::branch::update::BranchUpdate;
use crate::models::branch::Branch;
use crate::models::flow::DeleteFlow;
use crate::models::flow_step::DeleteFlowStep;
use crate::models::input_output::DeleteIo;
use crate::models::like::Like;
use crate::models::node::{Node, PrimaryKeyNode};
use crate::models::node_commit::NodeCommit;
use crate::models::node_counter::NodeCounter;
use crate::models::node_descendant::NodeDescendant;
use crate::models::traits::Branchable;
use crate::models::workflow::DeleteWorkflow;
use crate::services::elastic::{ElasticDocument, ElasticIndex};
use crate::utils::cloned_ref::ClonedRef;
use crate::utils::logger::log_error;
use charybdis::batch::{CharybdisBatch, CharybdisModelBatch, ModelBatch};
use charybdis::operations::Delete;
use charybdis::types::Uuid;
use elasticsearch::Elasticsearch;
use futures::{StreamExt, TryFutureExt};
use scylla::CachingSession;
use std::collections::HashMap;

impl Node {
    pub async fn delete_related_data(&self, req_data: &RequestData) -> Result<(), NodecosmosError> {
        NodeDelete::new(self, &req_data).run().await
    }

    pub async fn create_new_version_for_ancestors(&self, req_data: &RequestData) {
        let _ = NodeCommit::handle_deletion(req_data, &self)
            .map_err(|e| {
                log_error(format!("Error creating new version for node {}: {:?}", self.id, e));

                e
            })
            .await;
    }

    pub async fn update_branch_with_deletion(&self, req_data: &RequestData) {
        if self.is_branched() {
            Branch::update(
                &req_data.db_session(),
                self.branch_id,
                BranchUpdate::DeleteNode(self.id),
            )
            .await;
        }
    }
}

type Id = Uuid;
type OrderIndex = f64;
type Children = Vec<(Id, OrderIndex)>;
type ChildrenByParentId = HashMap<Uuid, Children>;

pub struct NodeDelete<'a> {
    db_session: &'a CachingSession,
    elastic_client: &'a Elasticsearch,
    node: &'a Node,
    node_ids_to_delete: Vec<Uuid>,
    children_by_parent_id: ChildrenByParentId,
}

impl<'a> NodeDelete<'a> {
    pub fn new(node: &'a Node, req_data: &'a RequestData) -> NodeDelete<'a> {
        Self {
            db_session: &req_data.app.db_session,
            elastic_client: &req_data.app.elastic_client,
            node,
            node_ids_to_delete: vec![node.id],
            children_by_parent_id: ChildrenByParentId::new(),
        }
    }

    pub async fn run(&mut self) -> Result<(), NodecosmosError> {
        self.populate_delete_data().await.map_err(|err| {
            log_error(format!("populate_delete_data: {}", err));
            return err;
        })?;

        self.delete_related_data().await.map_err(|err| {
            log_error(format!("delete_related_data: {}", err));
            return err;
        })?;

        self.delete_counter_data().await.map_err(|err| {
            log_error(format!("delete_counter_data: {}", err));
            return err;
        })?;

        self.delete_descendants().await.map_err(|err| {
            log_error(format!("delete_descendants: {}", err));
            return err;
        })?;

        self.delete_elastic_data().await;

        Ok(())
    }

    async fn populate_delete_data(&mut self) -> Result<(), NodecosmosError> {
        let mut descendants = self.node.descendants(self.db_session, None).await?;

        while let Some(descendant) = descendants.next().await {
            let descendant = descendant?;

            self.node_ids_to_delete.push(descendant.id);

            if !self.node.is_root {
                self.children_by_parent_id
                    .entry(descendant.parent_id)
                    .or_default()
                    .push((descendant.id, descendant.order_index));
            }
        }

        Ok(())
    }

    /// Here we delete workflows, flows, flow_steps, ios, likes, like_counts for node and its descendants.
    pub async fn delete_related_data(&mut self) -> Result<(), NodecosmosError> {
        for node_ids_chunk in self.node_ids_to_delete.chunks(100) {
            for id in node_ids_chunk {
                DeleteWorkflow {
                    node_id: *id,
                    ..Default::default()
                }
                .delete_by_partition_key()
                .execute(self.db_session)
                .await?;

                DeleteFlow {
                    node_id: *id,
                    ..Default::default()
                }
                .delete_by_partition_key()
                .execute(self.db_session)
                .await?;

                DeleteFlowStep {
                    node_id: *id,
                    ..Default::default()
                }
                .delete_by_partition_key()
                .execute(self.db_session)
                .await?;

                DeleteIo {
                    node_id: *id,
                    ..Default::default()
                }
                .delete_by_partition_key()
                .execute(self.db_session)
                .await?;

                Like {
                    object_id: *id,
                    ..Default::default()
                }
                .delete_by_partition_key()
                .execute(self.db_session)
                .await?;

                PrimaryKeyNode {
                    id: *id,
                    branch_id: self.node.branchise_id(*id),
                }
                .delete()
                .execute(self.db_session)
                .await?;
            }
        }

        Ok(())
    }

    //  Counter and non-counter mutations cannot exist in the same batch
    pub async fn delete_counter_data(&mut self) -> Result<(), NodecosmosError> {
        for node_ids_chunk in self.node_ids_to_delete.chunks(100) {
            let mut batch = NodeCounter::unlogged_batch();

            for id in node_ids_chunk {
                batch.append_delete(&NodeCounter {
                    id: *id,
                    branch_id: self.node.branchise_id(*id),
                    ..Default::default()
                })?;
            }

            batch.execute(self.db_session).await.map_err(|err| {
                return NodecosmosError::InternalServerError(format!(
                    "NodeDelete::delete_related_data: batch.execute: {}",
                    err
                ));
            })?;
        }

        Ok(())
    }

    /// Here we delete all of node descendants for all of its ancestors.
    /// We use child_ids_by_parent_id so we don't have to query ancestor_ids for each descendant node.
    /// Each node in the tree has its own descendant records in node_descendants table,
    /// so we have to get all of them.
    pub async fn delete_descendants(&mut self) -> Result<(), NodecosmosError> {
        if self.node.is_root {
            self.delete_tree().await?;
            return Ok(());
        }

        match self.node.parent_id {
            Some(parent_id) => {
                let order_index = self.node.order_index;

                self.children_by_parent_id
                    .entry(parent_id)
                    .or_default()
                    .push((self.node.id, order_index));

                let mut delete_stack = vec![parent_id];
                let mut current_ancestor_ids = self.node.ancestor_ids.cloned_ref();

                while let Some(parent_id) = delete_stack.pop() {
                    current_ancestor_ids.insert(parent_id);

                    let children: Children = self.children_by_parent_id.get(&parent_id).unwrap_or(&vec![]).clone();

                    for children_chunk in children.chunks(100) {
                        let mut batch = NodeDescendant::unlogged_delete_batch();

                        for (child_id, order_index) in children_chunk {
                            // delete node descendants for all of its ancestors
                            for ancestor_id in &current_ancestor_ids {
                                let branch_id = self.node.branchise_id(*ancestor_id);

                                batch.append_delete(&NodeDescendant {
                                    root_id: self.node.root_id,
                                    branch_id,
                                    node_id: *ancestor_id,
                                    order_index: *order_index,
                                    id: *child_id,
                                    ..Default::default()
                                })?;
                            }

                            // populate delete stack with child ids
                            delete_stack.push(*child_id);
                        }

                        batch.execute(self.db_session).await.map_err(|err| {
                            return NodecosmosError::InternalServerError(format!(
                                "NodeDelete::delete_descendants: batch.execute: {}",
                                err
                            ));
                        })?;
                    }
                }
            }
            None => {
                return Err(NodecosmosError::InternalServerError(
                    "NodeDelete::delete_descendants: parent_id is None".to_string(),
                ));
            }
        };

        Ok(())
    }

    async fn delete_tree(&self) -> Result<(), NodecosmosError> {
        NodeDescendant {
            root_id: self.node.id,
            ..Default::default()
        }
        .delete_by_partition_key()
        .execute(self.db_session)
        .await?;

        return Ok(());
    }

    async fn delete_elastic_data(&self) {
        if self.node.is_original() {
            Node::bulk_delete_elastic_documents(self.elastic_client, &self.node_ids_to_delete).await;
        }
    }
}
