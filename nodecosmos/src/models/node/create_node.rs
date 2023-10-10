use crate::app::CbExtension;
use crate::models::node::Node;
use crate::models::node_descendant::NodeDescendant;
use crate::services::elastic::add_elastic_document;
use charybdis::{CharybdisError, CharybdisModelBatch};
use scylla::CachingSession;

impl Node {
    pub async fn add_related_data(
        &self,
        db_session: &CachingSession,
        ext: &CbExtension,
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

            batch.execute(db_session).await?;
        }

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
