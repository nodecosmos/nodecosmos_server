use crate::errors::NodecosmosError;
use crate::models::flow::DeleteFlow;
use crate::models::flow_step::DeleteFlowStep;
use crate::models::input_output::DeleteIo;
use crate::models::like::Like;
use crate::models::likes_count::LikesCount;
use crate::models::node::{DeleteNode, Node};
use crate::models::node_descendant::NodeDescendant;
use crate::models::workflow::WorkDeleteFlow;
use crate::services::elastic::bulk_delete_elastic_documents;
use crate::services::logger::log_error;
use crate::CbExtension;
use charybdis::batch::CharybdisModelBatch;
use charybdis::operations::Delete;
use charybdis::types::Uuid;
use futures::StreamExt;
use scylla::CachingSession;
use std::collections::HashMap;

type Id = Uuid;
type OrderIndex = f64;
type ChildrenByParentId = HashMap<Uuid, Vec<(Id, OrderIndex)>>;

pub struct NodeDelete<'a> {
    node: &'a Node,
    db_session: &'a CachingSession,
    ext: &'a CbExtension,
    node_ids_to_delete: Vec<Id>,
    children_by_parent_id: ChildrenByParentId,
}

impl<'a> NodeDelete<'a> {
    pub fn new(node: &'a Node, db_session: &'a CachingSession, ext: &'a CbExtension) -> NodeDelete<'a> {
        Self {
            node,
            db_session,
            ext,
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

        self.delete_descendants().await.map_err(|err| {
            log_error(format!("delete_descendants: {}", err));
            return err;
        })?;

        self.delete_elastic_data().await;

        Ok(())
    }

    async fn populate_delete_data(&mut self) -> Result<(), NodecosmosError> {
        let descendants = self.node.descendants(self.db_session).await?;
        let mut chunked_stream = descendants.chunks(100);

        while let Some(descendants_chunk) = chunked_stream.next().await {
            for descendant in descendants_chunk {
                let descendant = descendant?;
                self.node_ids_to_delete.push(descendant.id);

                if !self.node.is_root {
                    self.children_by_parent_id
                        .entry(descendant.parent_id)
                        .or_default()
                        .push((descendant.id, descendant.order_index));
                }
            }
        }

        Ok(())
    }

    /// Here we delete workflows, flows, flow_steps, ios, likes, likes_counts for node and its descendants.
    pub async fn delete_related_data(&mut self) -> Result<(), NodecosmosError> {
        for node_ids_chunk in self.node_ids_to_delete.chunks(100) {
            let mut batch = CharybdisModelBatch::unlogged();
            for id in node_ids_chunk {
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

                batch.append_delete_by_partition_key(&LikesCount {
                    object_id: *id,
                    ..Default::default()
                })?;

                // root node will be deleted within callback
                if id != &self.node.id {
                    batch.append_delete(&DeleteNode { id: *id })?;
                }
            }
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

        println!("delete tree");

        return Ok(());
    }

    async fn delete_elastic_data(&self) {
        bulk_delete_elastic_documents(
            &self.ext.elastic_client,
            Node::ELASTIC_IDX_NAME,
            &self.node_ids_to_delete,
        )
        .await;
    }
}
