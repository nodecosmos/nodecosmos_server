use crate::errors::NodecosmosError;
use crate::models::node::{partial_node, Node};
use crate::services::elastic::update_elastic_document;
use crate::CbExtension;
use ammonia::clean;
use charybdis::*;
use scylla::CachingSession;

partial_node!(
    UpdateDescriptionNode,
    root_id,
    id,
    description,
    short_description,
    description_markdown,
    description_base64,
    updated_at
);

impl ExtCallbacks<CbExtension, NodecosmosError> for UpdateDescriptionNode {
    async fn before_update(
        &mut self,
        _db_session: &CachingSession,
        ext: &CbExtension,
    ) -> Result<(), NodecosmosError> {
        // sanitize description
        if let Some(description) = &self.description {
            self.description = Some(clean(description));
        }

        self.updated_at = Some(chrono::Utc::now());

        update_elastic_document(
            &ext.elastic_client,
            Node::ELASTIC_IDX_NAME,
            self,
            self.id.to_string(),
        )
        .await;
        Ok(())
    }
}
