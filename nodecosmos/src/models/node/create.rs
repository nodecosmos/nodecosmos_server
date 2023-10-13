use crate::models::node::Node;
use crate::models::node_descendant::NodeDescendant;
use crate::services::elastic::add_elastic_document;
use crate::services::logger::log_error;
use crate::CbExtension;
use charybdis::{CharybdisError, CharybdisModelBatch};
use scylla::CachingSession;

impl Node {
    pub async fn append_to_ancestors(
        &mut self,
        db_session: &CachingSession,
    ) -> Result<(), CharybdisError> {
        if let Some(ancestor_ids) = self.ancestor_ids.as_ref() {
            let mut batch = CharybdisModelBatch::new();

            for ancestor_id in ancestor_ids {
                let node_descendant = NodeDescendant {
                    root_id: self.root_id,
                    node_id: *ancestor_id,
                    id: self.id,
                    order_index: self.order_index.unwrap(),
                    parent_id: self.parent_id,
                    title: self.title.clone(),
                };

                batch.append_insert(&node_descendant)?;
            }

            batch.execute(db_session).await.map_err(|e| {
                log_error(format!(
                    "Error appending node {} to ancestors. {:?}",
                    self.id, e
                ));

                e
            })?;
        }

        Ok(())
    }
    pub async fn add_to_elastic(&self, ext: &CbExtension) -> Result<(), CharybdisError> {
        add_elastic_document(
            &ext.elastic_client,
            Node::ELASTIC_IDX_NAME,
            self,
            self.id.to_string(),
        )
        .await;

        Ok(())
    }
}
