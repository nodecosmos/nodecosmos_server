use crate::errors::NodecosmosError;
use crate::models::node::Node;
use crate::models::node_descendant::NodeDescendant;
use crate::models::udts::{Owner, OwnerTypes};
use crate::models::user::CurrentUser;
use crate::services::elastic::add_elastic_document;
use crate::services::logger::log_error;
use crate::CbExtension;
use charybdis::batch::CharybdisModelBatch;
use charybdis::errors::CharybdisError;
use charybdis::types::{Set, Uuid};
use chrono::Utc;
use scylla::CachingSession;

impl Node {
    pub fn set_owner(&mut self, current_user: &CurrentUser) {
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

    pub async fn append_to_ancestors(&self, db_session: &CachingSession) -> Result<(), CharybdisError> {
        if let Some(ancestor_ids) = self.ancestor_ids.as_ref() {
            let mut batch = CharybdisModelBatch::new();

            for ancestor_id in ancestor_ids {
                let node_descendant = NodeDescendant {
                    root_id: self.root_id,
                    node_id: *ancestor_id,
                    id: self.id,
                    order_index: self.order_index,
                    parent_id: self.parent_id.unwrap(),
                    title: self.title.clone(),
                };

                batch.append_insert(&node_descendant)?;
            }

            batch.execute(db_session).await.map_err(|e| {
                log_error(format!("Error appending node {} to ancestors. {:?}", self.id, e));

                e
            })?;
        }

        Ok(())
    }

    pub async fn add_to_elastic(&self, ext: &CbExtension) -> Result<(), CharybdisError> {
        add_elastic_document(&ext.elastic_client, Node::ELASTIC_IDX_NAME, self, self.id.to_string()).await;

        Ok(())
    }
}
