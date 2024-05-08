use anyhow::Context;
use charybdis::callbacks::Callbacks;
use charybdis::macros::charybdis_model;
use charybdis::model::AsNative;
use charybdis::stream::CharybdisModelStream;
use charybdis::types::{Boolean, Double, Frozen, Set, Text, Timestamp, Uuid};
use scylla::CachingSession;
use serde::{Deserialize, Serialize};

use nodecosmos_macros::{Branchable, Id, NodeAuthorization, NodeParent};

use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::branch::AuthBranch;
use crate::models::node::delete::NodeDelete;
use crate::models::traits::{Branchable, FindBranchedOrOriginalNode, NodeBranchParams};
use crate::models::traits::{Context as Ctx, ModelContext};
use crate::models::udts::Profile;

mod auth;
mod create;
pub mod delete;
pub mod reorder;
pub mod search;
pub mod sort;
pub mod update_cover_image;
mod update_owner;
mod update_title;

/// Note: All derives implemented bellow `charybdis_model` macro are automatically implemented for all partial models.
/// So `Branchable` derive is automatically applied to all partial_node models.
#[charybdis_model(
    table_name = nodes,
    partition_keys = [branch_id],
    clustering_keys = [id],
)]
#[derive(Branchable, NodeParent, NodeAuthorization, Id, Serialize, Deserialize, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Node {
    /// Original records are ones where branch_id == root_id.
    /// Branched are ones where branch_id == branch.id.
    #[serde(default)]
    pub branch_id: Uuid,

    #[serde(default)]
    pub id: Uuid,

    #[serde(default)]
    #[branch(original_id)]
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
    pub parent: Option<Box<Node>>,

    // For branched node we use branch fields to authenticate edits, not the original node.
    #[charybdis(ignore)]
    #[serde(skip)]
    pub auth_branch: Option<Box<AuthBranch>>,

    // Used only in case of merge undo to recover deleted data.
    // We should only delete single node in the ancestor chain as the time,
    // so we don't need to worry about large memory usage.
    #[charybdis(ignore)]
    #[serde(skip)]
    pub delete_data: Option<Box<NodeDelete>>,

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
        // In case of branch undo, we need to recover deleted data.
        // We shouldn't have duplicate delete_data for descendants as we only delete single node in the ancestor chain.
        // Note we consume delete_data here to avoid copying in next step
        if let Some(mut delete_data) = self.delete_data.take() {
            delete_data.recover(data).await?;
        }

        let self_clone = self.clone();
        let data = data.clone();

        tokio::spawn(async move {
            self_clone.add_to_elastic(data.elastic_client()).await;
            self_clone.create_workflow(&data).await;
        });

        Ok(())
    }

    async fn before_delete(&mut self, _: &CachingSession, data: &Self::Extension) -> Result<(), NodecosmosError> {
        *self = Node::find_branched_or_original(
            data.db_session(),
            NodeBranchParams {
                root_id: self.root_id,
                branch_id: self.branch_id,
                node_id: self.id,
            },
        )
        .await?;

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
    pub async fn is_original_deleted(
        db_session: &CachingSession,
        original_id: Uuid,
        id: Uuid,
    ) -> Result<bool, NodecosmosError> {
        let is_none = PkNode::maybe_find_first_by_branch_id_and_id(original_id, id)
            .execute(db_session)
            .await?
            .is_none();

        Ok(is_none)
    }

    pub async fn find_by_ids(
        db_session: &CachingSession,
        branch_id: Uuid,
        ids: &Set<Uuid>,
    ) -> Result<CharybdisModelStream<Node>, NodecosmosError> {
        let res = find_node!("branch_id = ? AND id IN ?", (branch_id, ids))
            .execute(db_session)
            .await?;

        Ok(res)
    }
}

partial_node!(PkNode, branch_id, id, root_id, owner_id, editor_ids, ancestor_ids);

impl PkNode {
    pub async fn find_by_ids(
        db_session: &CachingSession,
        branch_id: Uuid,
        ids: &[Uuid],
    ) -> Result<Vec<PkNode>, NodecosmosError> {
        let res = find_pk_node!("branch_id = ? AND id IN ?", (branch_id, ids))
            .execute(db_session)
            .await?
            .try_collect()
            .await?;

        Ok(res)
    }
}

partial_node!(
    BaseNode,
    branch_id,
    id,
    root_id,
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
    branch_id,
    id,
    root_id,
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

partial_node!(UpdateOrderNode, branch_id, id, root_id, parent_id, order_index);

partial_node!(
    UpdateTitleNode,
    branch_id,
    id,
    root_id,
    order_index,
    owner_id,
    editor_ids,
    viewer_ids,
    is_public,
    ancestor_ids,
    title,
    updated_at,
    parent_id,
    parent,
    auth_branch,
    ctx
);

impl Callbacks for UpdateTitleNode {
    type Extension = RequestData;
    type Error = NodecosmosError;

    async fn before_update(&mut self, _: &CachingSession, data: &Self::Extension) -> Result<(), NodecosmosError> {
        if self.is_branch() {
            self.as_native().create_branched_if_not_exist(data).await?;
            self.update_branch(&data).await?;
        }

        if !self.is_merge_context() {
            let title_clone = self.title.clone();

            // find_branched_or_original is used to get the latest data.
            *self = Self::find_branched_or_original(
                data.db_session(),
                NodeBranchParams {
                    root_id: self.root_id,
                    branch_id: self.branch_id,
                    node_id: self.id,
                },
            )
            .await?;

            self.title = title_clone;
        }

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

partial_node!(PrimaryKeyNode, branch_id, id, root_id);

partial_node!(
    AuthNode,
    branch_id,
    id,
    root_id,
    owner_id,
    editor_ids,
    viewer_ids,
    is_public,
    parent_id,
    parent,
    auth_branch
);

partial_node!(UpdateOwnerNode, branch_id, id, root_id, owner_id, owner, updated_at);

partial_node!(
    UpdateCoverImageNode,
    branch_id,
    id,
    root_id,
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
