use crate::app::CbExtension;
use crate::models::node::Node;
use crate::services::elastic::add_elastic_document;
use charybdis::{CharybdisError, CharybdisModelBatch};
use scylla::CachingSession;

impl Node {
    /// add node to parent's child_ids and ancestor's descendant_ids
    pub async fn add_related_data(
        &mut self,
        db_session: &CachingSession,
    ) -> Result<(), CharybdisError> {
        let mut batch = CharybdisModelBatch::new();

        if let Some(parent_id) = &self.parent_id {
            let query = Node::PUSH_TO_CHILD_IDS_QUERY;
            batch.append_statement(query, (self.id, self.root_id, parent_id))?;
        }

        if let Some(ancestor_ids) = &self.ancestor_ids {
            for ancestor_id in ancestor_ids {
                let query = Node::PUSH_TO_DESCENDANT_IDS_QUERY;
                batch.append_statement(query, (self.id, self.root_id, ancestor_id))?;
            }
        }

        batch.execute(db_session).await?;

        Ok(())
    }

    pub async fn add_to_elastic_index(&self, ext: &CbExtension) {
        add_elastic_document(
            &ext.elastic_client,
            Node::ELASTIC_IDX_NAME,
            self,
            self.id.to_string(),
        )
        .await;
    }
}
