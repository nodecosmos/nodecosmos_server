use crate::callback_extension::CbExtension;
use crate::errors::NodecosmosError;
use crate::models::node::delete::NodeDeleter;
use crate::models::node::{Node, UpdateCoverImageNode, UpdateDescriptionNode, UpdateLikesCountNode, UpdateTitleNode};
use crate::models::utils::impl_node_updated_at_with_elastic_ext_cb;
use crate::services::elastic::update_elastic_document;
use ammonia::clean;
use charybdis::callbacks::ExtCallbacks;
use scylla::CachingSession;

impl ExtCallbacks<CbExtension, NodecosmosError> for Node {
    async fn before_insert(&mut self, _: &CachingSession, _: &CbExtension) -> Result<(), NodecosmosError> {
        self.validate_root().await?;

        Ok(())
    }

    async fn after_insert(&self, db_session: &CachingSession, ext: &CbExtension) -> Result<(), NodecosmosError> {
        self.append_to_ancestors(db_session).await?;
        self.add_to_elastic(ext).await?;

        Ok(())
    }

    async fn before_delete(&mut self, db_session: &CachingSession, ext: &CbExtension) -> Result<(), NodecosmosError> {
        NodeDeleter::new(self, db_session, ext).run().await?;

        Ok(())
    }
}

impl ExtCallbacks<CbExtension, NodecosmosError> for UpdateDescriptionNode {
    async fn before_update(&mut self, _db_session: &CachingSession, ext: &CbExtension) -> Result<(), NodecosmosError> {
        // sanitize description
        if let Some(description) = &self.description {
            self.description = Some(clean(description));
        }

        self.updated_at = Some(chrono::Utc::now());

        update_elastic_document(&ext.elastic_client, Node::ELASTIC_IDX_NAME, self, self.id.to_string()).await;
        Ok(())
    }
}

impl ExtCallbacks<CbExtension, NodecosmosError> for UpdateTitleNode {
    async fn before_update(&mut self, session: &CachingSession, ext: &CbExtension) -> Result<(), NodecosmosError> {
        self.update_title_for_ancestors(session, ext).await?;
        self.update_elastic_index(ext).await;
        self.updated_at = Some(chrono::Utc::now());

        Ok(())
    }
}

impl_node_updated_at_with_elastic_ext_cb!(UpdateLikesCountNode);
impl_node_updated_at_with_elastic_ext_cb!(UpdateCoverImageNode);
