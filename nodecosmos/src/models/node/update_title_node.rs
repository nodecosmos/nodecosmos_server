use crate::errors::NodecosmosError;
use crate::models::helpers::ClonedRef;
use crate::models::node::{partial_node, Node};
use crate::models::node_descendant::update_node_descendant_query;
use crate::services::elastic::update_elastic_document;
use crate::CbExtension;
use charybdis::{CharybdisModelBatch, Double, ExtCallbacks, Set, Text, Timestamp, Uuid};
use chrono::Utc;
use scylla::CachingSession;

partial_node!(
    UpdateTitleNode,
    root_id,
    parent_id,
    id,
    title,
    ancestor_ids,
    order_index,
    updated_at
);

impl UpdateTitleNode {
    pub fn set_defaults(&mut self, native_node: Node) {
        self.updated_at = Some(Utc::now());

        self.root_id = native_node.root_id;
        self.parent_id = native_node.parent_id;
        self.order_index = native_node.order_index;
        self.ancestor_ids = native_node.ancestor_ids;
    }

    pub async fn update_node_descendants(
        &mut self,
        db_session: &CachingSession,
        ext: &CbExtension,
    ) -> Result<(), NodecosmosError> {
        let mut batch = CharybdisModelBatch::new();

        ext.resource_locker
            .lock(&self.root_id.to_string(), 1000)
            .await?;

        for ancestor_id in self.ancestor_ids.cloned_ref() {
            let update_title_query = update_node_descendant_query!("title = ?");

            batch.append_statement(
                update_title_query,
                (
                    self.title.clone(),
                    self.root_id,
                    ancestor_id,
                    self.order_index,
                    self.id,
                ),
            )?;
        }

        batch.execute(db_session).await?;

        ext.resource_locker
            .unlock(&self.root_id.to_string())
            .await?;

        Ok(())
    }

    pub async fn update_elastic_index(&self, ext: &CbExtension) {
        update_elastic_document(
            &ext.elastic_client,
            Node::ELASTIC_IDX_NAME,
            self,
            self.id.to_string(),
        )
        .await;
    }
}

impl ExtCallbacks<CbExtension, NodecosmosError> for UpdateTitleNode {
    async fn after_update(
        &mut self,
        session: &CachingSession,
        ext: &CbExtension,
    ) -> Result<(), NodecosmosError> {
        self.update_node_descendants(session, ext).await?;

        self.update_elastic_index(ext).await;

        Ok(())
    }
}
