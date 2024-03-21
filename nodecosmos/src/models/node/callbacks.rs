use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::node::context::Context;
use crate::models::node::{Node, UpdateCoverImageNode, UpdateDescriptionNode, UpdateLikesCountNode, UpdateTitleNode};
use crate::models::traits::{Branchable, MergeDescription, SanitizeDescription};
use charybdis::callbacks::Callbacks;
use charybdis::model::AsNative;
use charybdis::operations::Find;
use charybdis::options::Consistency;
use log::error;
use scylla::CachingSession;

impl Callbacks for Node {
    type Extension = RequestData;
    type Error = NodecosmosError;

    async fn before_insert(&mut self, db_session: &CachingSession, data: &RequestData) -> Result<(), NodecosmosError> {
        if &self.ctx != &Context::Merge && &self.ctx != &Context::BranchedInit {
            self.set_defaults(db_session).await?;
            self.set_owner(data).await?;
            self.validate_root().await?;
            self.validate_owner().await?;
        }

        self.preserve_ancestors_for_branch(data).await?;

        if let Err(e) = self.append_to_ancestors(db_session).await {
            self.remove_from_ancestors(db_session).await?;
            error!("[before_insert] Unexpected error updating branch with creation: {}", e);
        }

        if let Err(e) = self.update_branch_with_creation(data).await {
            self.remove_from_ancestors(db_session).await?;
            error!("[before_insert] Unexpected error updating branch with creation: {}", e);
        }

        Ok(())
    }

    async fn after_insert(&mut self, _: &CachingSession, data: &Self::Extension) -> Result<(), NodecosmosError> {
        let self_clone = self.clone();
        let data = data.clone();

        tokio::spawn(async move {
            self_clone.add_to_elastic(data.elastic_client()).await;
            self_clone.create_new_version(&data).await;
        });

        Ok(())
    }

    async fn before_delete(&mut self, _: &CachingSession, data: &Self::Extension) -> Result<(), NodecosmosError> {
        self.delete_related_data(data).await?;
        self.preserve_ancestors_for_branch(data).await?;
        self.preserve_descendants_for_branch(data).await?;
        self.update_branch_with_deletion(data).await?;

        Ok(())
    }

    async fn after_delete(&mut self, _: &CachingSession, data: &Self::Extension) -> Result<(), NodecosmosError> {
        self.create_branched_if_original_exist(&data).await?;

        let self_clone = self.clone();
        let data = data.clone();

        tokio::spawn(async move {
            self_clone.create_new_version_for_ancestors(&data).await;
        });

        Ok(())
    }
}

impl Callbacks for UpdateDescriptionNode {
    type Extension = RequestData;
    type Error = NodecosmosError;

    async fn before_update(
        &mut self,
        db_session: &CachingSession,
        data: &Self::Extension,
    ) -> Result<(), NodecosmosError> {
        if self.is_branched() {
            self.as_native().create_branched_if_not_exist(data).await?;
        }

        if &self.ctx != &Context::MergeRecovery {
            self.merge_description(db_session).await?;
        }

        self.description.sanitize()?;

        self.updated_at = Some(chrono::Utc::now());

        if self.is_branched() {
            let native = self
                .as_native()
                .find_by_primary_key()
                .consistency(Consistency::All)
                .execute(db_session)
                .await;
            match native {
                Ok(native) => {
                    native.preserve_ancestors_for_branch(data).await?;
                }
                Err(e) => error!("[after_update] Unexpected error finding native node: {}", e),
            }
        }

        self.update_branch(&data).await?;

        Ok(())
    }

    async fn after_update(&mut self, _: &CachingSession, data: &Self::Extension) -> Result<(), NodecosmosError> {
        let self_clone = self.clone();
        let data = data.clone();

        tokio::spawn(async move {
            self_clone.update_elastic_index(data.elastic_client()).await;
            self_clone.create_new_version(&data).await;
        });

        Ok(())
    }
}

impl Callbacks for UpdateTitleNode {
    type Extension = RequestData;
    type Error = NodecosmosError;

    async fn before_update(&mut self, _: &CachingSession, data: &Self::Extension) -> Result<(), NodecosmosError> {
        if self.is_branched() {
            self.as_native().create_branched_if_not_exist(data).await?;
        }

        self.update_branch(&data).await?;
        self.updated_at = Some(chrono::Utc::now());

        Ok(())
    }

    async fn after_update(&mut self, _: &CachingSession, data: &Self::Extension) -> Result<(), NodecosmosError> {
        self.update_title_for_ancestors(data).await?;

        let self_clone = self.clone();
        let data = data.clone();

        tokio::spawn(async move {
            self_clone.update_elastic_index(data.elastic_client()).await;
            self_clone.create_new_version(&data).await;
        });

        Ok(())
    }
}

macro_rules! impl_node_updated_at_with_elastic_ext_cb {
    ($struct_name:ident) => {
        impl charybdis::callbacks::Callbacks for $struct_name {
            type Extension = crate::api::data::RequestData;
            type Error = crate::errors::NodecosmosError;

            async fn before_update(
                &mut self,
                _db_session: &CachingSession,
                _data: &Self::Extension,
            ) -> Result<(), NodecosmosError> {
                self.updated_at = Some(chrono::Utc::now());

                Ok(())
            }

            async fn after_update(
                &mut self,
                _db_session: &CachingSession,
                data: &Self::Extension,
            ) -> Result<(), NodecosmosError> {
                use crate::models::traits::ElasticDocument;

                if self.id != self.branch_id {
                    return Ok(());
                }

                self.update_elastic_document(data.elastic_client()).await;

                Ok(())
            }
        }
    };
}

impl_node_updated_at_with_elastic_ext_cb!(UpdateLikesCountNode);
impl_node_updated_at_with_elastic_ext_cb!(UpdateCoverImageNode);
