use crate::app::CbExtension;
use crate::models::flow::FlowDelete;
use crate::models::flow_step::FlowStepDelete;
use crate::models::input_output::IoDelete;
use crate::models::node::{DeleteNode, Node};
use crate::models::node_descendant::NodeDescendant;
use crate::models::workflow::WorkflowDelete;
use crate::services::elastic::bulk_delete_elastic_documents;
use charybdis::{CharybdisError, CharybdisModelBatch, Find};
use scylla::CachingSession;

impl Node {
    // delete descendant nodes, workflows, flows, and flow steps as single batch
    pub async fn delete_related_data(
        &self,
        db_session: &CachingSession,
        ext: &CbExtension,
    ) -> Result<(), CharybdisError> {
        let mut node_ids_to_delete = vec![self.id];

        let descendants = NodeDescendant {
            root_id: self.id,
            ..Default::default()
        }
        .find_by_partition_key(db_session)
        .await?;

        let descendant_ids = descendants.iter().map(|descendant| descendant.id);
        node_ids_to_delete.extend(descendant_ids);

        let node_ids_to_delete_chunks = node_ids_to_delete.chunks(100);

        // delete nodes in chunks of 100
        for node_ids in node_ids_to_delete_chunks {
            let mut batch = CharybdisModelBatch::new();

            for id in node_ids {
                // remove node from ancestor's descendant_ids
                let mut node = Node {
                    id: *id,
                    ..Default::default()
                }
                .find_by_primary_key(db_session)
                .await?;

                node.remove_from_ancestors(db_session).await?;

                // remove workflows, flows, flow steps, and ios
                let workflows_to_delete = WorkflowDelete {
                    node_id: *id,
                    ..Default::default()
                }
                .find_by_partition_key(db_session)
                .await?;

                let flows_to_delete = FlowDelete {
                    node_id: *id,
                    ..Default::default()
                }
                .find_by_partition_key(db_session)
                .await?;

                let flow_steps_to_delete = FlowStepDelete {
                    node_id: *id,
                    ..Default::default()
                }
                .find_by_partition_key(db_session)
                .await?;

                let ios_to_delete = IoDelete {
                    node_id: *id,
                    ..Default::default()
                }
                .find_by_partition_key(db_session)
                .await?;

                batch.append_deletes(ios_to_delete)?;
                batch.append_deletes(flow_steps_to_delete)?;
                batch.append_deletes(flows_to_delete)?;
                batch.append_deletes(workflows_to_delete)?;
                batch.append_delete(&DeleteNode { id: *id })?;
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
