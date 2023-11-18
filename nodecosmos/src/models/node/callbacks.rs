use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::node::{Node, UpdateCoverImageNode, UpdateDescriptionNode, UpdateLikesCountNode, UpdateTitleNode};
use crate::models::utils::impl_node_updated_at_with_elastic_ext_cb;
use charybdis::callbacks::ExtCallbacks;
use scylla::CachingSession;

impl ExtCallbacks for Node {
    type Extension = RequestData;
    type Error = NodecosmosError;

    async fn before_insert(&mut self, db_session: &CachingSession, _: &RequestData) -> Result<(), NodecosmosError> {
        self.set_defaults(db_session).await?;
        self.validate_root().await?;
        self.validate_owner().await?;

        Ok(())
    }

    async fn after_insert(&mut self, _: &CachingSession, ext: &RequestData) -> Result<(), NodecosmosError> {
        let self_clone = self.clone();
        let ext = ext.clone();

        tokio::spawn(async move {
            let db_session = &ext.app.db_session;
            let app = &ext.app;

            let _ = self_clone.append_to_ancestors(&db_session).await;
            let _ = self_clone.add_to_elastic(&app).await;
            self_clone.create_new_version(&ext).await;
        });

        Ok(())
    }

    async fn before_delete(&mut self, _: &CachingSession, ext: &RequestData) -> Result<(), NodecosmosError> {
        self.delete_related_data(&ext).await?;

        Ok(())
    }

    async fn after_delete(&mut self, _session: &CachingSession, ext: &Self::Extension) -> Result<(), NodecosmosError> {
        let self_clone = self.clone();
        let ext = ext.clone();

        tokio::spawn(async move {
            self_clone.create_new_version_for_ancestors(&ext).await;
        });

        Ok(())
    }
}

impl ExtCallbacks for UpdateDescriptionNode {
    type Extension = RequestData;
    type Error = NodecosmosError;

    async fn before_update(&mut self, _db_session: &CachingSession, _ext: &RequestData) -> Result<(), NodecosmosError> {
        self.sanitize_description();
        self.updated_at = Some(chrono::Utc::now());

        Ok(())
    }

    async fn after_update(&mut self, _db_session: &CachingSession, ext: &RequestData) -> Result<(), NodecosmosError> {
        let self_clone = self.clone();
        let ext = ext.clone();

        tokio::spawn(async move {
            let _ = self_clone.update_elastic_index(&ext.app).await;
            let _ = self_clone.create_new_version(&ext).await;
        });

        Ok(())
    }
}

impl ExtCallbacks for UpdateTitleNode {
    type Extension = RequestData;
    type Error = NodecosmosError;

    async fn before_update(&mut self, session: &CachingSession, ext: &RequestData) -> Result<(), NodecosmosError> {
        self.update_title_for_ancestors(session, &ext.app).await?;
        self.updated_at = Some(chrono::Utc::now());

        Ok(())
    }

    async fn after_update(&mut self, _db_session: &CachingSession, ext: &RequestData) -> Result<(), NodecosmosError> {
        let self_clone = self.clone();
        let ext = ext.clone();

        tokio::spawn(async move {
            let _ = self_clone.update_elastic_index(&ext.app).await;
            let _ = self_clone.create_new_version(&ext).await;
        });

        Ok(())
    }
}

impl_node_updated_at_with_elastic_ext_cb!(UpdateLikesCountNode);
impl_node_updated_at_with_elastic_ext_cb!(UpdateCoverImageNode);
