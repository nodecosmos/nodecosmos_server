use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::branch::branchable::Branchable;
use crate::models::branch::update::BranchUpdate;
use crate::models::branch::Branch;
use crate::models::node::Node;
use crate::models::node_commit::NodeCommit;
use crate::models::node_descendant::NodeDescendant;
use crate::models::udts::Owner;
use crate::services::elastic::add_elastic_document;
use crate::services::elastic::index::ElasticIndex;
use crate::utils::cloned_ref::ClonedRef;
use crate::utils::logger::log_error;
use charybdis::batch::CharybdisModelBatch;
use charybdis::callbacks::ExtCallbacks;
use charybdis::operations::{Find, Insert, InsertWithExtCallbacks};
use charybdis::types::{Set, Uuid};
use chrono::Utc;
use elasticsearch::Elasticsearch;
use futures::TryFutureExt;
use scylla::CachingSession;

impl Node {
    pub async fn set_owner(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        if self.is_original() {
            let current_user = &data.current_user;
            let owner = Owner::init(current_user);

            self.owner_id = Some(owner.id);
            self.owner_type = Some(owner.owner_type.clone());

            self.owner = Some(owner);
        } else {
            if let Some(parent) = self.parent(data.db_session()).await? {
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

        if let Some(parent) = self.parent(session).await? {
            let editor_ids = parent.editor_ids.clone().unwrap_or(Set::new());
            let root_id = parent.root_id;
            let is_public = parent.is_public;
            let parent_id = parent.id;
            let mut ancestor_ids = parent.ancestor_ids.clone().unwrap_or(Set::new());
            ancestor_ids.insert(parent.id);

            self.root_id = root_id;
            self.parent_id = Some(parent_id);
            self.editor_ids = Some(editor_ids);
            self.is_public = is_public;
            self.is_root = false;
            self.ancestor_ids = Some(ancestor_ids);
        } else {
            self.root_id = self.id;
            self.parent_id = None;
            self.order_index = 0.0;
            self.ancestor_ids = None;
        }

        if self.branch_id == Uuid::default() {
            self.branch_id = self.id;
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
                    branch_id: self.branchise_id(*ancestor_id),
                    order_index: self.order_index,
                    parent_id: self.parent_id.expect("parent_id must be present"),
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

    /// When we create node within branch (Contribution Request),
    /// we need to create branched version for all ancestors,
    /// so we can preserve branch changes in case any of ancestor is deleted,
    /// and we can allow users to resolve conflicts.
    pub async fn preserve_ancestors_for_branch(&self, data: &RequestData) {
        if self.is_branched() {
            let mut futures = vec![];

            for ancestor_id in self.ancestor_ids.cloned_ref() {
                let ancestor = Node {
                    id: ancestor_id,
                    branch_id: self.branchise_id(ancestor_id),
                    ..Default::default()
                };
                let future = ancestor.create_branched_if_not_exist(data);

                futures.push(future);
            }

            futures::future::join_all(futures).await;
        }
    }

    /// Create branched version of self if it doesn't exist.
    pub async fn create_branched_if_not_exist(self, data: &RequestData) {
        let self_branched = self.find_by_primary_key(data.db_session()).await.ok();

        if self_branched.is_none() {
            let branch_id = self.branch_id;
            let non_branched_res = Node {
                branch_id: self.id,
                ..self
            }
            .find_by_primary_key(data.db_session())
            .await;

            match non_branched_res {
                Ok(mut self_branched) => {
                    self_branched.branch_id = branch_id;
                    let insert = self_branched.insert(data.db_session()).await;

                    if let Err(e) = insert {
                        log_error(format!(
                            "Error creating branched node {} from non-branched node {}: {:?}",
                            self_branched.id, self.id, e
                        ));
                    }

                    self_branched.append_to_ancestors(data.db_session()).await;
                    self_branched.create_new_version(&data).await;
                }
                Err(e) => {
                    log_error(format!("Error finding non-branched node {}: {:?}", self.id, e));
                }
            }
        }
    }

    pub async fn add_to_elastic(&self, elastic_client: &Elasticsearch) {
        if self.is_original() {
            add_elastic_document(elastic_client, Self::ELASTIC_IDX_NAME, self, self.id.to_string()).await;
        }
    }

    pub async fn create_new_version(&self, req_data: &RequestData) {
        let _ = NodeCommit::handle_creation(&req_data.db_session(), &self, req_data.current_user_id())
            .map_err(|e| {
                log_error(format!("Error creating new version for node {}: {:?}", self.id, e));

                e
            })
            .await;
    }

    pub async fn update_branch(&self, req_data: &RequestData) {
        if self.is_branched() {
            Branch::update(
                &req_data.db_session(),
                self.branch_id,
                BranchUpdate::CreateNode(self.id),
            )
            .await;
        }
    }
}
