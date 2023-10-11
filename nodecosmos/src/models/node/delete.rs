use crate::errors::NodecosmosError;
use crate::models::flow::FlowDelete;
use crate::models::flow_step::FlowStepDelete;
use crate::models::input_output::IoDelete;
use crate::models::like::Like;
use crate::models::likes_count::LikesCount;
use crate::models::node::{DeleteNode, Node};
use crate::models::node_descendant::NodeDescendant;
use crate::models::workflow::WorkflowDelete;
use crate::services::elastic::bulk_delete_elastic_documents;
use crate::services::logger::log_error;
use crate::CbExtension;
use charybdis::{CharybdisModelBatch, Uuid};
use futures::StreamExt;
use scylla::CachingSession;
use std::collections::HashMap;

type Id = Uuid;
type OrderIndex = f64;
type ChildrenByParentId = HashMap<Uuid, Vec<(Id, OrderIndex)>>;

pub struct NodeDeleter<'a> {
    node: &'a Node,
    db_session: &'a CachingSession,
    ext: &'a CbExtension,
    node_ids_to_delete: Vec<Id>,
    children_by_parent_id: ChildrenByParentId,
}

impl<'a> NodeDeleter<'a> {
    pub fn new(
        node: &'a Node,
        db_session: &'a CachingSession,
        ext: &'a CbExtension,
    ) -> NodeDeleter<'a> {
        Self {
            node,
            db_session,
            ext,
            node_ids_to_delete: vec![node.id],
            children_by_parent_id: ChildrenByParentId::new(),
        }
    }

    pub async fn run(&mut self) -> Result<(), NodecosmosError> {
        self.delete_and_populate_desc_data().await.map_err(|err| {
            log_error(format!("delete_and_populate: {}", err));
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

    /// Here we delete workflows, flows, flow_steps, ios, likes, likes_counts for node and all of its descendants.
    /// Here we also consume stream of descendants, so we populate children_by_parent_id that
    /// is used in self.delete_descendants.
    pub async fn delete_and_populate_desc_data(&mut self) -> Result<(), NodecosmosError> {
        let descendants = self.node.descendants(self.db_session).await?;
        let mut chunked_stream = descendants.chunks(100);

        while let Some(descendants_chunk) = chunked_stream.next().await {
            let mut batch = CharybdisModelBatch::unlogged();

            for descendant in descendants_chunk {
                let descendant = descendant?;

                self.node_ids_to_delete.push(descendant.id);

                batch.append_delete_by_partition_key(&WorkflowDelete {
                    node_id: descendant.id,
                    ..Default::default()
                })?;

                batch.append_delete_by_partition_key(&FlowDelete {
                    node_id: descendant.id,
                    ..Default::default()
                })?;

                batch.append_delete_by_partition_key(&FlowStepDelete {
                    node_id: descendant.id,
                    ..Default::default()
                })?;

                batch.append_delete_by_partition_key(&IoDelete {
                    node_id: descendant.id,
                    ..Default::default()
                })?;

                batch.append_delete_by_partition_key(&Like {
                    object_id: descendant.id,
                    ..Default::default()
                })?;

                batch.append_delete(&DeleteNode { id: descendant.id })?;

                if let Some(parent_id) = descendant.parent_id {
                    self.children_by_parent_id
                        .entry(parent_id)
                        .or_default()
                        .push((descendant.id, descendant.order_index));
                }
            }

            batch.execute(self.db_session).await?;
        }

        Ok(())
    }

    /// In Scylla counter and non-counter data can not be part of the same batch
    pub async fn delete_counter_data(&mut self) -> Result<(), NodecosmosError> {
        for node_ids_chunk in self.node_ids_to_delete.chunks(100) {
            let mut batch = CharybdisModelBatch::unlogged();

            for node_id in node_ids_chunk {
                batch.append_delete_by_partition_key(&LikesCount {
                    object_id: *node_id,
                    ..Default::default()
                })?;
            }

            batch.execute(self.db_session).await?;
        }

        Ok(())
    }

    /// Here we delete all of node descendants for all of its ancestors.
    /// We use child_ids_by_parent_id so we don't have to query ancestor_ids for each descendant node.
    /// It's important to remember that each node in tree has its own descendant records in node_descendants table,
    /// so we have to get all of them.
    pub async fn delete_descendants(&mut self) -> Result<(), NodecosmosError> {
        let order_index = self.node.order_index.unwrap_or_default();
        let start_from_parent_id = self.node.parent_id.unwrap_or(self.node.id);

        // populate with current node
        self.children_by_parent_id
            .entry(start_from_parent_id)
            .or_default()
            .push((self.node.id, order_index));

        let mut delete_stack = vec![start_from_parent_id];
        let mut current_ancestor_ids = self.node.ancestor_ids.clone().unwrap_or_default();

        while let Some(parent_id) = delete_stack.pop() {
            current_ancestor_ids.push(parent_id);

            let child_ids_and_indices: Vec<(Id, OrderIndex)> = self
                .children_by_parent_id
                .get(&parent_id)
                .unwrap_or(&vec![])
                .clone();

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

                batch.execute(self.db_session).await?;
            }
        }

        Ok(())
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
