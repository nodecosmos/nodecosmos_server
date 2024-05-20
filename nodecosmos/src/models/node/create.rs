use charybdis::batch::ModelBatch;
use charybdis::operations::{Find, Insert};
use charybdis::types::{Set, Uuid};
use elasticsearch::Elasticsearch;
use futures::{stream, StreamExt, TryFutureExt};
use log::error;
use scylla::CachingSession;

use crate::api::data::RequestData;
use crate::constants::MAX_PARALLEL_REQUESTS;
use crate::errors::NodecosmosError;
use crate::models::branch::update::BranchUpdate;
use crate::models::branch::Branch;
use crate::models::node::Node;
use crate::models::node_descendant::NodeDescendant;
use crate::models::traits::ModelContext;
use crate::models::traits::Parent;
use crate::models::traits::RefCloned;
use crate::models::traits::{Branchable, ElasticDocument};
use crate::models::udts::Profile;
use crate::models::workflow::Workflow;

impl Node {
    pub async fn set_defaults(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        self.id = Uuid::new_v4();

        if let Some(parent) = self.parent(data.db_session()).await? {
            let editor_ids = parent.editor_ids.clone();
            let viewer_ids = parent.viewer_ids.clone();
            let root_id = parent.root_id;
            let is_public = parent.is_public;
            let parent_id = parent.id;
            let owner_id = parent.owner_id;
            let mut ancestor_ids = parent.ancestor_ids.clone().unwrap_or(Set::new());
            let owner = parent.owner.clone();
            ancestor_ids.insert(parent.id);

            self.root_id = root_id;
            self.parent_id = Some(parent_id);
            self.editor_ids = editor_ids;
            self.viewer_ids = viewer_ids;
            self.is_public = is_public;
            self.is_root = false;
            self.ancestor_ids = Some(ancestor_ids);
            self.owner_id = owner_id;
            self.owner = owner;
        } else {
            self.owner_id = Some(data.current_user.id);
            self.owner = Some(Profile::init_from_current_user(&data.current_user));
            self.root_id = self.id;
            self.parent_id = None;
            self.order_index = 0.0;
            self.ancestor_ids = None;
        }

        if self.branch_id == Uuid::default() {
            self.branch_id = self.root_id;
        }

        Ok(())
    }

    pub fn validate_root(&mut self) -> Result<(), NodecosmosError> {
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

    pub fn validate_owner(&mut self) -> Result<(), NodecosmosError> {
        if self.owner_id.is_none() || self.owner.is_none() {
            return Err(NodecosmosError::ValidationError((
                "owner_id".to_string(),
                "must be present".to_string(),
            )));
        }

        Ok(())
    }

    pub async fn append_to_ancestors(&self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        if let Some(ancestor_ids) = self.ancestor_ids.as_ref() {
            let mut descendants = Vec::with_capacity(ancestor_ids.len());

            for ancestor_id in ancestor_ids {
                let node_descendant = NodeDescendant {
                    root_id: self.root_id,
                    node_id: *ancestor_id,
                    id: self.id,
                    branch_id: self.branch_id,
                    order_index: self.order_index,
                    parent_id: self.parent_id.expect("parent_id must be present"),
                    title: self.title.clone(),
                };

                descendants.push(node_descendant);
            }

            if let Err(e) = NodeDescendant::batch()
                .append_inserts(&descendants)
                .execute(db_session)
                .await
            {
                error!(
                    "[append_to_ancestors::chunked_insert] Error for node {}: {:?}",
                    self.id, e
                );

                return Err(NodecosmosError::from(e));
            }
        }

        Ok(())
    }

    /// When we create node within branch (Contribution Request),
    /// we need to create branched version for all ancestors,
    /// so we can preserve branch changes in case any of ancestor is deleted,
    /// and we can allow users to resolve conflicts.
    pub async fn preserve_branch_ancestors(&self, data: &RequestData) -> Result<(), NodecosmosError> {
        if self.is_branch() {
            let mut futures = vec![];

            for ancestor_id in self.ancestor_ids.ref_cloned() {
                let ancestor = Node {
                    id: ancestor_id,
                    branch_id: self.branch_id,
                    root_id: self.root_id,
                    ..Default::default()
                };
                let future = ancestor.create_branched_if_not_exist(&data);

                futures.push(future);
            }

            let futures_stream = stream::iter(futures);

            for future in futures_stream
                .buffer_unordered(MAX_PARALLEL_REQUESTS)
                .collect::<Vec<Result<(), NodecosmosError>>>()
                .await
                .into_iter()
            {
                if let Err(e) = future {
                    error!(
                        "[preserve_branch_ancestors] Error preserving ancestors for branch: {:?}",
                        e
                    );

                    return Err(e);
                }
            }
        }

        Ok(())
    }

    pub async fn create_branched_if_not_exist(self, data: &RequestData) -> Result<(), NodecosmosError> {
        let self_branched_res = self.maybe_find_by_primary_key().execute(data.db_session()).await?;

        if self_branched_res.is_none() {
            let non_branched_res = Node::find_by_branch_id_and_id(self.original_id(), self.id)
                .execute(data.db_session())
                .await;

            match non_branched_res {
                Ok(mut node) => {
                    node.branch_id = self.branch_id;
                    node.set_branched_init_context();
                    let insert = node.insert().execute(data.db_session()).await;

                    if let Err(e) = insert {
                        error!(
                            "Error creating branched node -> id: {} branch_id: {} from non-branched node: {:?}",
                            node.id, self.branch_id, e
                        );
                    }

                    node.append_to_ancestors(data.db_session()).await?;
                    node.maybe_create_workflow(&data).await?;
                }
                Err(e) => {
                    error!(
                        "Error finding non-branched node {}: original_id: {} {:?}",
                        self.id,
                        self.original_id(),
                        e
                    );

                    return Err(NodecosmosError::from(e));
                }
            }
        }

        Ok(())
    }

    pub async fn preserve_branch_descendants(&self, data: &RequestData) -> Result<(), NodecosmosError> {
        if self.is_branch() {
            let mut descendants =
                NodeDescendant::find_by_root_id_and_branch_id_and_node_id(self.root_id, self.id, self.id)
                    .execute(data.db_session())
                    .await?;

            let mut futures = vec![];

            while let Some(descendant) = descendants.next().await {
                let descendant = descendant?;
                let descendant_node = Node {
                    id: descendant.id,
                    branch_id: self.branch_id,
                    ..Default::default()
                };
                let future = descendant_node.create_branched_if_not_exist(&data);

                futures.push(future);
            }

            let futures_stream = stream::iter(futures);

            for future in futures_stream
                .buffer_unordered(MAX_PARALLEL_REQUESTS)
                .collect::<Vec<Result<(), NodecosmosError>>>()
                .await
                .into_iter()
            {
                if let Err(e) = future {
                    error!(
                        "[preserve_branch_descendants] Error preserving descendants for branch: {:?}",
                        e
                    );

                    return Err(e);
                }
            }
        }

        Ok(())
    }

    // used to preserve deleted node for branch
    pub async fn create_branched_if_original_exist(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        if self.is_branch() {
            let original_node = Node {
                id: self.id,
                branch_id: self.original_id(),
                root_id: self.root_id,
                ..Default::default()
            }
            .maybe_find_by_primary_key()
            .execute(data.db_session())
            .await?;

            if let Some(mut new_branched) = original_node {
                new_branched.branch_id = self.branch_id;
                let res = new_branched.insert().execute(data.db_session()).await;

                if let Err(err) = res {
                    error!("Node::preserve_branched_if_original_exist::new_branched_res {}", err);

                    return Err(NodecosmosError::from(err));
                }

                self.append_to_ancestors(data.db_session()).await?;
            }
        };

        Ok(())
    }

    pub async fn add_to_elastic(&self, elastic_client: &Elasticsearch) {
        if self.is_original() {
            self.add_elastic_document(elastic_client).await;
        }
    }

    pub async fn create_workflow(&self, data: &RequestData) -> Result<(), NodecosmosError> {
        Workflow {
            root_id: self.root_id,
            node_id: self.id,
            branch_id: self.branch_id,
            title: Some(format!("{} Workflow", self.title)),
            created_at: self.created_at,
            updated_at: self.updated_at,
            initial_input_ids: None,
            ctx: self.ctx,
        }
        .insert()
        .execute(data.db_session())
        .map_err(|e| {
            error!("Error creating new workflow for node {}: {:?}", self.id, e);

            e
        })
        .await?;

        Ok(())
    }

    pub async fn maybe_create_workflow(&self, data: &RequestData) -> Result<(), NodecosmosError> {
        let maybe_branched = Workflow::maybe_find_first_by_branch_id_and_node_id(self.branch_id, self.id)
            .execute(data.db_session())
            .await?;

        if maybe_branched.is_none() {
            Workflow {
                root_id: self.root_id,
                node_id: self.id,
                branch_id: self.branch_id,
                title: Some(format!("{} Workflow", self.title)),
                ..Default::default()
            }
            .insert()
            .execute(data.db_session())
            .await?;
        }

        Ok(())
    }

    pub async fn update_branch_with_creation(&self, data: &RequestData) -> Result<(), NodecosmosError> {
        if self.is_branch() {
            Branch::update(
                data.db_session(),
                self.branch_id,
                BranchUpdate::CreateNode((self.id, self.ancestor_ids.clone().unwrap_or_default())),
            )
            .await?;
        }

        Ok(())
    }
}
