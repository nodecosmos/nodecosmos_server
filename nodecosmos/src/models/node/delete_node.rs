use crate::app::CbExtension;
use crate::elastic::bulk_delete_elastic_documents;
use crate::models::flow::FlowDelete;
use crate::models::flow_step::FlowStepDelete;
use crate::models::input_output::IoDelete;
use crate::models::node::{DeleteNode, Node};
use crate::models::workflow::WorkflowDelete;
use charybdis::{CharybdisError, CharybdisModelBatch, Find};
use scylla::CachingSession;

impl Node {
    // delete descendant nodes, workflows, flows, and flow steps as single batch
    pub async fn delete_related_data(
        &self,
        db_session: &CachingSession,
    ) -> Result<(), CharybdisError> {
        let mut batch = CharybdisModelBatch::new();
        let mut node_ids_to_delete = vec![self.id];

        let ancestor_ids = self.ancestor_ids.clone().unwrap_or_default();

        if let Some(descendant_ids) = &self.descendant_ids {
            node_ids_to_delete.extend(descendant_ids);
        }

        if let Some(parent_id) = &self.parent_id {
            let query = Node::PULL_FROM_CHILD_IDS_QUERY;
            batch.append_statement(query, (self.id, self.root_id, parent_id))?;
        }

        for id in node_ids_to_delete {
            // remove node from ancestor's descendant_ids
            for ancestor_id in &ancestor_ids {
                let query = Node::PULL_FROM_DESCENDANT_IDS_QUERY;
                batch.append_statement(query, (id, self.root_id, ancestor_id))?;
            }

            // remove workflows, flows, flow steps, and ios
            let workflows_to_delete = WorkflowDelete {
                node_id: id,
                ..Default::default()
            }
            .find_by_partition_key(db_session)
            .await?;

            let flows_to_delete = FlowDelete {
                node_id: id,
                ..Default::default()
            }
            .find_by_partition_key(db_session)
            .await?;

            let flow_steps_to_delete = FlowStepDelete {
                node_id: id,
                ..Default::default()
            }
            .find_by_partition_key(db_session)
            .await?;

            let ios_to_delete = IoDelete {
                node_id: id,
                ..Default::default()
            }
            .find_by_partition_key(db_session)
            .await?;

            batch.append_deletes(ios_to_delete)?;
            batch.append_deletes(flow_steps_to_delete)?;
            batch.append_deletes(flows_to_delete)?;
            batch.append_deletes(workflows_to_delete)?;
            batch.append_delete(DeleteNode {
                id,
                root_id: self.root_id,
            })?;
        }

        batch.execute(db_session).await
    }

    pub async fn delete_related_elastic_data(&self, ext: &CbExtension) {
        let mut node_ids_to_delete = vec![self.id];

        if let Some(descendant_ids) = &self.descendant_ids {
            node_ids_to_delete.extend(descendant_ids);
        }

        bulk_delete_elastic_documents(
            &ext.elastic_client,
            Node::ELASTIC_IDX_NAME,
            node_ids_to_delete,
        )
        .await;
    }
}
