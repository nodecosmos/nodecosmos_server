use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::flow::DeleteFlow;
use crate::models::flow_step::DeleteFlowStep;
use crate::models::input_output::DeleteIo;
use crate::models::like::Like;
use crate::models::likes_count::LikesCount;
use crate::models::node::{DeleteNode, Node};
use crate::models::node_commit::NodeCommit;
use crate::models::node_descendant::NodeDescendant;
use crate::models::workflow::WorkDeleteFlow;
use crate::services::elastic::bulk_delete_elastic_documents;
use crate::services::elastic::index::ElasticIndex;
use crate::utils::logger::log_error;
use charybdis::batch::CharybdisModelBatch;
use charybdis::operations::Delete;
use charybdis::types::Uuid;
use elasticsearch::Elasticsearch;
use futures::{StreamExt, TryFutureExt};
use scylla::CachingSession;
use std::collections::HashMap;

type Id = Uuid;
type BranchId = Uuid;
type OrderIndex = f64;
type ChildrenByParentId = HashMap<Uuid, Vec<(Id, OrderIndex)>>;

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
}

pub struct NodeDelete<'a> {
    db_session: &'a CachingSession,
    elastic_client: &'a Elasticsearch,
    node: &'a Node,
    node_ids_to_delete: Vec<(Id, BranchId)>,
    children_by_parent_id: ChildrenByParentId,
}

impl<'a> NodeDelete<'a> {
    pub fn new(node: &'a Node, req_data: &'a RequestData) -> NodeDelete<'a> {
        Self {
            db_session: &req_data.app.db_session,
            elastic_client: &req_data.app.elastic_client,
            node,
            node_ids_to_delete: vec![(node.id, node.branch_id)],
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
        let mut descendants = self.node.descendants(self.db_session).await?;

        while let Some(descendant) = descendants.next().await {
            let descendant = descendant?;

            self.node_ids_to_delete.push((descendant.id, descendant.branch_id));

            if !self.node.is_root {
                self.children_by_parent_id
                    .entry(descendant.parent_id)
                    .or_default()
                    .push((descendant.id, descendant.order_index));
            }
        }

        Ok(())
    }

    /// Here we delete workflows, flows, flow_steps, ios, likes, likes_counts for node and its descendants.
    pub async fn delete_related_data(&mut self) -> Result<(), NodecosmosError> {
        for node_ids_chunk in self.node_ids_to_delete.chunks(100) {
            let mut batch = CharybdisModelBatch::unlogged();

            for (id, branch_id) in node_ids_chunk {
                batch.append_delete_by_partition_key(&WorkDeleteFlow {
                    node_id: *id,
                    ..Default::default()
                })?;

                batch.append_delete_by_partition_key(&DeleteFlow {
                    node_id: *id,
                    ..Default::default()
                })?;

                batch.append_delete_by_partition_key(&DeleteFlowStep {
                    node_id: *id,
                    ..Default::default()
                })?;

                batch.append_delete_by_partition_key(&DeleteIo {
                    node_id: *id,
                    ..Default::default()
                })?;

                batch.append_delete_by_partition_key(&Like {
                    object_id: *id,
                    ..Default::default()
                })?;

                // self.node will be deleted within callback
                if id != &self.node.id {
                    batch.append_delete(&DeleteNode {
                        id: *id,
                        branch_id: *branch_id,
                    })?;
                }
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

    //  Counter and non-counter mutations cannot exist in the same batch
    pub async fn delete_counter_data(&mut self) -> Result<(), NodecosmosError> {
        for node_ids_chunk in self.node_ids_to_delete.chunks(100) {
            let mut batch = CharybdisModelBatch::unlogged();

            for (id, _) in node_ids_chunk {
                batch.append_delete_by_partition_key(&LikesCount {
                    object_id: *id,
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
                let mut current_ancestor_ids = self.node.ancestor_ids.clone().unwrap_or(vec![]);

                while let Some(parent_id) = delete_stack.pop() {
                    current_ancestor_ids.push(parent_id);

                    let child_ids_and_indices: Vec<(Id, OrderIndex)> =
                        self.children_by_parent_id.get(&parent_id).unwrap_or(&vec![]).clone();

                    for child_ids_and_indices_chunk in child_ids_and_indices.chunks(100) {
                        let mut batch = CharybdisModelBatch::unlogged();

                        for (child_id, order_index) in child_ids_and_indices_chunk {
                            // delete node descendants for all of its ancestors
                            for ancestor_id in &current_ancestor_ids {
                                batch.append_delete(&NodeDescendant {
                                    root_id: self.node.root_id,
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

        println!("delete delete_descendants");

        Ok(())
    }

    async fn delete_tree(&self) -> Result<(), NodecosmosError> {
        NodeDescendant {
            root_id: self.node.id,
            ..Default::default()
        }
        .delete_by_partition_key(self.db_session)
        .await?;

        return Ok(());
    }

    async fn delete_elastic_data(&self) {
        if self.node.is_main_branch() {
            let node_ids_to_delete = self.node_ids_to_delete.iter().map(|(id, _)| *id).collect::<Vec<Uuid>>();
            bulk_delete_elastic_documents(self.elastic_client, Node::ELASTIC_IDX_NAME, &node_ids_to_delete).await;
        }
    }
}
