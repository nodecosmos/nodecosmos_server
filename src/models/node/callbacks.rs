use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::node::{Node, UpdateCoverImageNode, UpdateDescriptionNode, UpdateLikesCountNode, UpdateTitleNode};
use crate::models::utils::impl_node_updated_at_with_elastic_ext_cb;
use charybdis::callbacks::ExtCallbacks;
use scylla::CachingSession;

impl ExtCallbacks for Node {
    type Extension = RequestData;
    type Error = NodecosmosError;

    async fn before_insert(&mut self, db_session: &CachingSession, data: &RequestData) -> Result<(), NodecosmosError> {
        if !self.merge {
            self.set_defaults(db_session).await?;
            self.set_owner(data).await?;
            self.validate_root().await?;
            self.validate_owner().await?;
        }

        Ok(())
    }

    async fn after_insert(&mut self, _: &CachingSession, req_data: &RequestData) -> Result<(), NodecosmosError> {
        let self_clone = self.clone();
        let req_data = req_data.clone();

        tokio::spawn(async move {
            self_clone.append_to_ancestors(req_data.db_session()).await;
            self_clone.add_to_elastic(req_data.elastic_client()).await;
            self_clone.create_new_version(&req_data).await;
            self_clone.update_branch(&req_data).await;
        });

        Ok(())
    }

    async fn before_delete(&mut self, _: &CachingSession, req_data: &RequestData) -> Result<(), NodecosmosError> {
        self.delete_related_data(&req_data).await?;

        Ok(())
    }

    async fn after_delete(
        &mut self,
        _session: &CachingSession,
        req_data: &Self::Extension,
    ) -> Result<(), NodecosmosError> {
        let self_clone = self.clone();
        let req_data = req_data.clone();

        tokio::spawn(async move {
            self_clone.create_new_version_for_ancestors(&req_data).await;
            self_clone.update_branch_with_deletion(&req_data).await;
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

    async fn after_update(
        &mut self,
        _db_session: &CachingSession,
        req_data: &RequestData,
    ) -> Result<(), NodecosmosError> {
        let self_clone = self.clone();
        let req_data = req_data.clone();

        tokio::spawn(async move {
            self_clone.update_elastic_index(req_data.elastic_client()).await;
            self_clone.create_new_version(&req_data).await;
            self_clone.update_branch(&req_data).await;
        });

        Ok(())
    }
}

impl ExtCallbacks for UpdateTitleNode {
    type Extension = RequestData;
    type Error = NodecosmosError;

    async fn before_update(&mut self, _session: &CachingSession, _ext: &RequestData) -> Result<(), NodecosmosError> {
        self.updated_at = Some(chrono::Utc::now());

        Ok(())
    }

    async fn after_update(
        &mut self,
        _db_session: &CachingSession,
        req_data: &RequestData,
    ) -> Result<(), NodecosmosError> {
        let self_clone = self.clone();
        let req_data = req_data.clone();

        tokio::spawn(async move {
            self_clone.update_title_for_ancestors(&req_data).await;
            self_clone.update_elastic_index(req_data.elastic_client()).await;
            self_clone.create_new_version(&req_data).await;
            self_clone.update_branch(&req_data).await;
        });

        Ok(())
    }
}

impl_node_updated_at_with_elastic_ext_cb!(UpdateLikesCountNode);
impl_node_updated_at_with_elastic_ext_cb!(UpdateCoverImageNode);
