use anyhow::Context;
use charybdis::callbacks::Callbacks;
use charybdis::macros::charybdis_model;
use charybdis::model::AsNative;
use charybdis::operations::Find;
use charybdis::stream::CharybdisModelStream;
use charybdis::types::{Boolean, Double, Frozen, Set, Text, Timestamp, Uuid};
use scylla::CachingSession;
use serde::{Deserialize, Serialize};

use nodecosmos_macros::{Branchable, Id, NodeAuthorization, NodeParent};

use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::branch::AuthBranch;
use crate::models::traits::{Branchable, FindBranchedOrOriginal};
use crate::models::traits::{Context as Ctx, ModelContext};
use crate::models::udts::Profile;

mod auth;
mod create;
mod delete;
pub mod reorder;
pub mod search;
pub mod sort;
pub mod update_cover_image;
mod update_owner;
mod update_title;

/// Note: All derives implemented bellow `charybdis_model` macro are automatically implemented for all partial models.
/// So `Authorization` trait is implemented within `NodeAuthorization` and it's automatically implemented for all
/// partial models if they have `auth_branch` field.
#[charybdis_model(
    table_name = nodes,
    partition_keys = [id],
    clustering_keys = [branch_id],
)]
#[derive(Branchable, NodeParent, NodeAuthorization, Id, Serialize, Deserialize, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Node {
    #[serde(default)]
    pub branch_id: Uuid,

    #[serde(default)]
    #[branch(original_id)]
    pub id: Uuid,

    #[serde(default)]
    pub root_id: Uuid,

    #[serde(default)]
    pub is_public: Boolean,

    #[serde(default)]
    pub is_root: Boolean,

    #[serde(default)]
    pub order_index: Double,

    pub title: Text,
    pub parent_id: Option<Uuid>,
    pub ancestor_ids: Option<Set<Uuid>>,
    pub owner_id: Option<Uuid>,
    pub owner: Option<Frozen<Profile>>,
    pub editor_ids: Option<Set<Uuid>>,
    pub viewer_ids: Option<Set<Uuid>>,
    pub cover_image_filename: Option<Text>,
    pub cover_image_url: Option<Text>,

    #[serde(default = "chrono::Utc::now")]
    pub created_at: Timestamp,

    #[serde(default = "chrono::Utc::now")]
    pub updated_at: Timestamp,

    #[charybdis(ignore)]
    #[serde(skip)]
    pub parent: Option<BaseNode>,

    #[charybdis(ignore)]
    #[serde(skip)]
    pub auth_branch: Option<AuthBranch>,

    #[charybdis(ignore)]
    #[serde(skip)]
    pub ctx: Ctx,
}

impl Callbacks for Node {
    type Extension = RequestData;
    type Error = NodecosmosError;

    async fn before_insert(&mut self, db_session: &CachingSession, data: &RequestData) -> Result<(), NodecosmosError> {
        if self.is_default_context() {
            self.set_defaults(db_session).await?;
            self.set_owner(data).await?;
            self.validate_root()?;
            self.validate_owner()?;
            self.update_branch_with_creation(data)
                .await
                .context("Failed to update branch")?;
        }

        self.preserve_branch_ancestors(data).await?;
        self.append_to_ancestors(db_session).await?;

        Ok(())
    }

    async fn after_insert(&mut self, _: &CachingSession, data: &Self::Extension) -> Result<(), NodecosmosError> {
        let self_clone = self.clone();
        let data = data.clone();

        tokio::spawn(async move {
            self_clone.add_to_elastic(data.elastic_client()).await;
            self_clone.create_workflow(&data).await;
        });

        Ok(())
    }

    async fn before_delete(&mut self, _: &CachingSession, data: &Self::Extension) -> Result<(), NodecosmosError> {
        *self = Node::find_branched_or_original(data.db_session(), self.id, self.branch_id).await?;

        self.preserve_branch_ancestors(data).await?;
        self.preserve_branch_descendants(data).await?;
        self.update_branch_with_deletion(data).await?;
        self.archive_and_delete(data).await?;

        Ok(())
    }

    async fn after_delete(&mut self, _: &CachingSession, data: &Self::Extension) -> Result<(), NodecosmosError> {
        self.create_branched_if_original_exist(&data).await?;

        Ok(())
    }
}

impl Node {
    pub async fn is_original_deleted(db_session: &CachingSession, id: Uuid) -> Result<bool, NodecosmosError> {
        let is_none = PkNode::maybe_find_first_by_id_and_branch_id(id, id)
            .execute(db_session)
            .await?
            .is_none();

        Ok(is_none)
    }

    pub async fn find_by_ids_and_branch_id(
        db_session: &CachingSession,
        ids: &Set<Uuid>,
        branch_id: Uuid,
    ) -> Result<CharybdisModelStream<Node>, NodecosmosError> {
        let res = find_node!("id IN ? AND branch_id = ?", (ids, branch_id))
            .execute(db_session)
            .await?;

        Ok(res)
    }

    pub async fn find_by_ids(
        db_session: &CachingSession,
        ids: &Set<Uuid>,
    ) -> Result<CharybdisModelStream<Node>, NodecosmosError> {
        let res = find_node!("id IN ? AND branch_id IN ?", (ids, ids))
            .execute(db_session)
            .await?;

        Ok(res)
    }
}

partial_node!(PkNode, id, branch_id, owner_id, editor_ids, ancestor_ids);

impl PkNode {
    pub async fn find_by_ids(db_session: &CachingSession, ids: &[Uuid]) -> Result<Vec<PkNode>, NodecosmosError> {
        let res = find_pk_node!("id IN ? AND branch_id IN ?", (ids, ids))
            .execute(db_session)
            .await?
            .try_collect()
            .await?;

        Ok(res)
    }

    pub async fn find_by_ids_and_branch_id(
        db_session: &CachingSession,
        ids: &[Uuid],
        branch_id: Uuid,
    ) -> Result<Vec<PkNode>, NodecosmosError> {
        let res = find_pk_node!("id IN ? AND branch_id = ?", (ids, branch_id))
            .execute(db_session)
            .await?
            .try_collect()
            .await?;

        Ok(res)
    }
}

partial_node!(
    BaseNode,
    root_id,
    id,
    branch_id,
    owner_id,
    editor_ids,
    viewer_ids,
    is_root,
    ancestor_ids,
    order_index,
    title,
    parent_id,
    owner,
    is_public,
    cover_image_url,
    created_at,
    updated_at,
    auth_branch
);

partial_node!(
    GetStructureNode,
    root_id,
    id,
    branch_id,
    owner_id,
    editor_ids,
    viewer_ids,
    is_public,
    parent_id,
    ancestor_ids,
    order_index,
    parent,
    auth_branch,
    ctx
);

partial_node!(UpdateOrderNode, id, branch_id, parent_id, order_index);

partial_node!(
    UpdateTitleNode,
    id,
    branch_id,
    order_index,
    owner_id,
    editor_ids,
    viewer_ids,
    is_public,
    ancestor_ids,
    title,
    updated_at,
    root_id,
    parent_id,
    parent,
    auth_branch,
    ctx
);

impl Callbacks for UpdateTitleNode {
    type Extension = RequestData;
    type Error = NodecosmosError;

    async fn before_update(&mut self, _: &CachingSession, data: &Self::Extension) -> Result<(), NodecosmosError> {
        if self.is_branched() {
            self.as_native().create_branched_if_not_exist(data).await?;
            self.update_branch(&data).await?;
        }

        let current = Self::find_branched_or_original(data.db_session(), self.id, self.branch_id).await?;

        self.root_id = current.root_id;
        self.ancestor_ids = current.ancestor_ids;
        self.order_index = current.order_index;
        self.parent_id = current.parent_id;

        self.update_title_for_ancestors(data).await?;

        Ok(())
    }

    async fn after_update(&mut self, _: &CachingSession, data: &Self::Extension) -> Result<(), NodecosmosError> {
        let self_clone = self.clone();
        let data = data.clone();

        tokio::spawn(async move {
            self_clone.update_elastic_index(data.elastic_client()).await;
        });

        Ok(())
    }
}

partial_node!(PrimaryKeyNode, id, branch_id);

partial_node!(
    AuthNode,
    id,
    branch_id,
    owner_id,
    editor_ids,
    viewer_ids,
    is_public,
    parent_id,
    parent,
    auth_branch
);

partial_node!(UpdateOwnerNode, id, branch_id, owner_id, owner, updated_at);

partial_node!(
    UpdateCoverImageNode,
    id,
    branch_id,
    cover_image_filename,
    cover_image_url,
    updated_at
);

impl Callbacks for UpdateCoverImageNode {
    type Extension = RequestData;
    type Error = NodecosmosError;

    async fn after_update(&mut self, _: &CachingSession, data: &Self::Extension) -> Result<(), NodecosmosError> {
        use crate::models::traits::ElasticDocument;

        if self.id != self.branch_id {
            return Ok(());
        }

        self.update_elastic_document(data.elastic_client()).await;

        Ok(())
    }
}
