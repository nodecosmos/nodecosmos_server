use crate::app::CbExtension;
use crate::models::flow::FlowDelete;
use crate::models::flow_step::FlowStepDelete;
use crate::models::input_output::IoDelete;
use crate::models::like::Like;
use crate::models::likes_count::LikesCount;
use crate::models::node::{find_node_query, DeleteNode, Node};
use crate::models::workflow::WorkflowDelete;
use crate::services::elastic::bulk_delete_elastic_documents;
use charybdis::{CharybdisError, CharybdisModelBatch, Delete, Find};
use scylla::CachingSession;

impl Node {
    // delete descendant nodes, workflows, flows, and flow steps as single batch
    pub async fn delete_related_data(
        &self,
        db_session: &CachingSession,
        ext: &CbExtension,
    ) -> Result<(), CharybdisError> {
        let mut node_ids_to_delete = vec![self.id];

        let descendants = self.descendants(db_session).await?;

        let descendant_ids = descendants.iter().map(|descendant| descendant.id);
        node_ids_to_delete.extend(descendant_ids);

        let node_ids_to_delete_chunks = node_ids_to_delete.chunks(100);

        // delete nodes in chunks of 100
        for node_ids in node_ids_to_delete_chunks {
            let mut batch = CharybdisModelBatch::new();

            // remove node from ancestors' descendants
            let nodes = Node::find(db_session, find_node_query!("id IN ?"), (node_ids,)).await?;

            for mut node in nodes.flatten() {
                // this will also remove the nodes descendants from self.descendants
                node.remove_from_ancestors(db_session).await?;

                batch.append_delete_by_partition_key(&WorkflowDelete {
                    node_id: node.id,
                    ..Default::default()
                })?;
                batch.append_delete_by_partition_key(&FlowDelete {
                    node_id: node.id,
                    ..Default::default()
                })?;
                batch.append_delete_by_partition_key(&FlowStepDelete {
                    node_id: node.id,
                    ..Default::default()
                })?;
                batch.append_delete_by_partition_key(&IoDelete {
                    node_id: node.id,
                    ..Default::default()
                })?;

                batch.append_delete_by_partition_key(&Like {
                    object_id: node.id,
                    ..Default::default()
                })?;
                LikesCount {
                    object_id: node.id,
                    ..Default::default()
                }
                .delete_by_partition_key(db_session)
                .await?;

                batch.append_delete(&DeleteNode { id: node.id })?;
            }

            batch.execute(db_session).await?;
        }

        bulk_delete_elastic_documents(
            &ext.elastic_client,
            Node::ELASTIC_IDX_NAME,
            node_ids_to_delete,
        )
        .await;

        Ok(())
    }
}
