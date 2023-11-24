use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::node::Node;
use crate::models::node_commit::NodeCommit;
use crate::models::node_descendant::NodeDescendant;
use crate::models::udts::{Owner, OwnerTypes};
use crate::services::elastic::add_elastic_document;
use crate::services::elastic::index::ElasticIndex;
use crate::utils::logger::log_error;
use charybdis::batch::CharybdisModelBatch;
use charybdis::types::{Set, Uuid};
use chrono::Utc;
use elasticsearch::Elasticsearch;
use futures::TryFutureExt;
use scylla::CachingSession;

impl Node {
    pub async fn set_owner(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        if self.is_main_branch_node() {
            let current_user = &data.current_user;

            let owner = Owner {
                id: current_user.id,
                name: current_user.full_name(),
                username: Some(current_user.username.clone()),
                owner_type: OwnerTypes::User.into(),
                profile_image_url: None,
            };

            self.owner_id = Some(owner.id);
            self.owner_type = Some(owner.owner_type.clone());

            self.owner = Some(owner);
        } else {
            if let Some(parent) = self.parent(data.db_session()).await?.as_ref() {
                if let Some(owner) = parent.owner.as_ref() {
                    let owner = Owner::init_from(owner);
                    self.owner_id = Some(owner.id);
                    self.owner_type = Some(owner.owner_type.clone());

                    self.owner = Some(owner);
                } else {
                    return Err(NodecosmosError::ValidationError((
                        "parent.owner".to_string(),
                        "must be present".to_string(),
                    )));
                }
            } else {
                return Err(NodecosmosError::ValidationError((
                    "parent_id".to_string(),
                    "must be present".to_string(),
                )));
            }
        }

        Ok(())
    }

    pub async fn set_defaults(&mut self, session: &CachingSession) -> Result<(), NodecosmosError> {
        self.id = Uuid::new_v4();
        let parent = self.parent(session).await?;

        if let Some(parent) = parent.as_ref() {
            self.root_id = parent.root_id;
            self.parent_id = Some(parent.id);
            self.editor_ids = parent.editor_ids.clone();
            self.is_public = parent.is_public;
            self.is_root = false;

            let mut ancestor_ids = parent.ancestor_ids.clone().unwrap_or(Set::new());
            ancestor_ids.push(parent.id);
            self.ancestor_ids = Some(ancestor_ids);
        } else {
            self.root_id = self.id;
            self.parent_id = None;
            self.order_index = 0.0;
            self.ancestor_ids = None;
        }

        let now = Utc::now();

        self.created_at = Some(now);
        self.updated_at = Some(now);

        Ok(())
    }

    pub async fn validate_root(&mut self) -> Result<(), NodecosmosError> {
        if self.is_root {
            if self.root_id != self.id {
                return Err(NodecosmosError::ValidationError((
                    "root_id".to_string(),
                    "must be equal to id".to_string(),
                )));
            }
        } else {
            if self.root_id == Uuid::default() || self.root_id == self.id {
                return Err(NodecosmosError::ValidationError((
                    "root_id".to_string(),
                    "is invalid".to_string(),
                )));
            }
        }

        Ok(())
    }

    pub async fn validate_owner(&mut self) -> Result<(), NodecosmosError> {
        if self.owner_id.is_none() || self.owner.is_none() {
            return Err(NodecosmosError::ValidationError((
                "owner_id".to_string(),
                "must be present".to_string(),
            )));
        }

        if self.owner_type.is_none() {
            return Err(NodecosmosError::ValidationError((
                "owner_type".to_string(),
                "must be present".to_string(),
            )));
        }

        Ok(())
    }

    pub async fn append_to_ancestors(&self, db_session: &CachingSession) {
        if let Some(ancestor_ids) = self.ancestor_ids.as_ref() {
            let mut batch = CharybdisModelBatch::unlogged();

            for ancestor_id in ancestor_ids {
                let node_descendant = NodeDescendant {
                    root_id: self.root_id,
                    node_id: *ancestor_id,
                    id: self.id,
                    branch_id: self.branch_id,
                    order_index: self.order_index,
                    parent_id: self.parent_id.unwrap(),
                    title: self.title.clone(),
                };

                let _ = batch.append_insert(&node_descendant).map_err(|e| {
                    log_error(format!(
                        "Error appending node {} to ancestor {} descendants. {:?}",
                        self.id, ancestor_id, e
                    ));

                    e
                });
            }

            let _ = batch
                .execute(db_session)
                .map_err(|e| {
                    log_error(format!("Error appending node {} to ancestors. {:?}", self.id, e));

                    e
                })
                .await;
        }
    }

    pub async fn add_to_elastic(&self, elastic_client: &Elasticsearch) -> Result<(), NodecosmosError> {
        if self.is_main_branch_node() {
            add_elastic_document(elastic_client, Self::ELASTIC_IDX_NAME, self, self.id.to_string()).await;
        }

        Ok(())
    }

    pub async fn create_new_version(&self, req_data: &RequestData) {
        let _ = NodeCommit::handle_creation(&req_data.db_session(), &self, req_data.current_user_id())
            .map_err(|e| {
                log_error(format!("Error creating new version for node {}: {:?}", self.id, e));

                e
            })
            .await;
    }
}
