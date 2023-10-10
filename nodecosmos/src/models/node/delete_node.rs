use crate::app::CbExtension;
use crate::models::flow::FlowDelete;
use crate::models::flow_step::FlowStepDelete;
use crate::models::input_output::IoDelete;
use crate::models::like::Like;
use crate::models::likes_count::LikesCount;
use crate::models::node::{DeleteNode, Node};
use crate::models::node_descendant::NodeDescendant;
use crate::models::workflow::WorkflowDelete;
use crate::services::elastic::bulk_delete_elastic_documents;
use charybdis::{CharybdisError, CharybdisModelBatch, Delete, Uuid};
use futures::StreamExt;
use scylla::CachingSession;
use std::collections::HashMap;

type Id = Uuid;
type OrderIndex = f64;
type ChildIdsAndIndicesByParentId = HashMap<Uuid, Vec<(Id, OrderIndex)>>;

impl Node {
    // delete descendant nodes, workflows, flows, and flow steps as single batch
    pub async fn delete_dependent_data(
        &mut self,
        db_session: &CachingSession,
        ext: &CbExtension,
    ) -> Result<(), CharybdisError> {
        let descendants = self.descendants(db_session).await?;

        // delete node data in chunks of 100
        let mut chunked_stream = descendants.chunks(100);
        let mut node_ids_to_delete = vec![self.id];
        let mut child_ids_and_indices_by_parent_id = ChildIdsAndIndicesByParentId::new();

        while let Some(descendants_chunk) = chunked_stream.next().await {
            let mut batch = CharybdisModelBatch::unlogged();

            for descendant in descendants_chunk {
                let descendant = descendant?;

                node_ids_to_delete.push(descendant.id);

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

                LikesCount {
                    object_id: descendant.id,
                    ..Default::default()
                }
                .delete_by_partition_key(db_session)
                .await?;

                if descendant.id != self.id {
                    batch.append_delete(&DeleteNode { id: descendant.id })?;
                }

                if let Some(parent_id) = descendant.parent_id {
                    child_ids_and_indices_by_parent_id
                        .entry(parent_id)
                        .or_default()
                        .push((descendant.id, descendant.order_index));
                }
            }

            batch.execute(db_session).await?;
        }

        // Here we delete node descendants records along with their descendants.
        let order_index = self.order_index.unwrap_or_default();

        // We can safely start from parent_id because in upper loop we only append descendants
        // of the current node to the `child_ids_and_indices_by_parent_id`.
        let start_from_parent_id = self.parent_id.unwrap_or(self.id);

        // populate with current node
        child_ids_and_indices_by_parent_id
            .entry(start_from_parent_id)
            .or_default()
            .push((self.id, order_index));

        let mut delete_stack = vec![start_from_parent_id];
        let mut current_ancestor_ids = self.ancestor_ids.clone().unwrap_or_default();

        while let Some(parent_id) = delete_stack.pop() {
            current_ancestor_ids.push(parent_id);

            let child_ids_and_indices: Vec<(Id, OrderIndex)> = child_ids_and_indices_by_parent_id
                .get(&parent_id)
                .unwrap_or(&vec![])
                .clone();

            for child_ids_and_indices_chunk in child_ids_and_indices.chunks(100) {
                let mut batch = CharybdisModelBatch::unlogged();

                for (child_id, order_index) in child_ids_and_indices_chunk {
                    // delete node descendants for all of its ancestors
                    for ancestor_id in &current_ancestor_ids {
                        batch.append_delete(&NodeDescendant {
                            root_id: self.root_id,
                            node_id: *ancestor_id,
                            order_index: *order_index,
                            id: *child_id,
                            ..Default::default()
                        })?;
                    }

                    // populate delete stack with child ids
                    delete_stack.push(*child_id);
                }

                batch.execute(db_session).await?;
            }
        }

        // delete from elastic
        bulk_delete_elastic_documents(
            &ext.elastic_client,
            Node::ELASTIC_IDX_NAME,
            node_ids_to_delete,
        )
        .await;

        Ok(())
    }
}
