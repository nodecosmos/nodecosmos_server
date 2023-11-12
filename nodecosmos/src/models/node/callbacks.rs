use crate::callback_extension::CbExtension;
use crate::errors::NodecosmosError;
use crate::models::node::delete::NodeDelete;
use crate::models::node::{Node, UpdateCoverImageNode, UpdateDescriptionNode, UpdateLikesCountNode, UpdateTitleNode};
use crate::models::utils::impl_node_updated_at_with_elastic_ext_cb;
use charybdis::callbacks::ExtCallbacks;
use scylla::CachingSession;

impl ExtCallbacks<CbExtension, NodecosmosError> for Node {
    async fn before_insert(&mut self, db_session: &CachingSession, _: &CbExtension) -> Result<(), NodecosmosError> {
        self.set_defaults(db_session).await?;
        self.validate_root().await?;
        self.validate_owner().await?;

        Ok(())
    }

    async fn after_insert(&mut self, db_session: &CachingSession, ext: &CbExtension) -> Result<(), NodecosmosError> {
        self.append_to_ancestors(db_session).await?;
        self.add_to_elastic(ext).await?;

        Ok(())
    }

    async fn before_delete(&mut self, db_session: &CachingSession, ext: &CbExtension) -> Result<(), NodecosmosError> {
        NodeDelete::new(self, db_session, ext).run().await?;

        Ok(())
    }
}

impl ExtCallbacks<CbExtension, NodecosmosError> for UpdateDescriptionNode {
    async fn before_update(&mut self, _db_session: &CachingSession, _ext: &CbExtension) -> Result<(), NodecosmosError> {
        self.sanitize_description();
        self.updated_at = Some(chrono::Utc::now());

        Ok(())
    }

    async fn after_update(&mut self, _db_session: &CachingSession, ext: &CbExtension) -> Result<(), NodecosmosError> {
        self.update_elastic_index(ext).await;

        Ok(())
    }
}

impl ExtCallbacks<CbExtension, NodecosmosError> for UpdateTitleNode {
    async fn before_update(&mut self, session: &CachingSession, ext: &CbExtension) -> Result<(), NodecosmosError> {
        self.update_title_for_ancestors(session, ext).await?;
        self.updated_at = Some(chrono::Utc::now());

        Ok(())
    }

    async fn after_update(&mut self, _db_session: &CachingSession, ext: &CbExtension) -> Result<(), NodecosmosError> {
        self.update_elastic_index(ext).await;

        Ok(())
    }
}

impl_node_updated_at_with_elastic_ext_cb!(UpdateLikesCountNode);
impl_node_updated_at_with_elastic_ext_cb!(UpdateCoverImageNode);
