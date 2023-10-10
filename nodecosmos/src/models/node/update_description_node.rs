use crate::errors::NodecosmosError;
use crate::models::helpers::sanitize_description_ext_cb_fn;
use crate::models::node::{partial_node, Node};
use crate::services::elastic::update_elastic_document;
use crate::CbExtension;
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
    sanitize_description_ext_cb_fn!();

    async fn after_update(
        &mut self,
        _db_session: &CachingSession,
        ext: &CbExtension,
    ) -> Result<(), NodecosmosError> {
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
