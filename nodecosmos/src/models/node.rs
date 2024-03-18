mod auth;
pub mod callbacks;
pub mod context;
mod create;
mod delete;
pub mod reorder;
pub mod search;
pub mod sort;
pub mod update_cover_image;
mod update_description;
mod update_owner;
mod update_title;

use crate::errors::NodecosmosError;
use crate::models::branch::AuthBranch;
use crate::models::node::context::Context;
use crate::models::traits::Branchable;
use crate::models::udts::Profile;
use crate::utils::defaults::default_to_0;
use charybdis::macros::charybdis_model;
use charybdis::operations::Find;
use charybdis::types::{BigInt, Boolean, Double, Frozen, Set, Text, Timestamp, Uuid};
use nodecosmos_macros::{Branchable, Id, NodeAuthorization, NodeParent, RootId};
use scylla::statement::Consistency;
use scylla::CachingSession;
use serde::{Deserialize, Serialize};

/// Note: All derives implemented bellow `charybdis_model` macro are automatically implemented for all partial models.
/// So `Authorization` trait is implemented within `NodeAuthorization` and it's automatically implemented for all partial models
/// if they have `auth_branch` field.
#[charybdis_model(
    table_name = nodes,
    partition_keys = [id],
    clustering_keys = [branch_id],
)]
#[derive(NodeParent, NodeAuthorization, Id, RootId, Branchable, Serialize, Deserialize, Default, Clone)]
pub struct Node {
    #[serde(default)]
    pub id: Uuid,

    // `self.branch_id` is equal to `self.id` for the main node's branch
    #[serde(default, rename = "branchId")]
    pub branch_id: Uuid,

    #[serde(default, rename = "rootId")]
    pub root_id: Uuid,

    #[serde(rename = "versionId")]
    pub version_id: Option<Uuid>,

    #[serde(rename = "isPublic", default)]
    pub is_public: Boolean,

    #[serde(rename = "isRoot")]
    pub is_root: Boolean,

    #[serde(rename = "order", default)]
    pub order_index: Double,

    pub title: Text,

    #[serde(rename = "parentId")]
    pub parent_id: Option<Uuid>,

    #[serde(rename = "ancestorIds")]
    pub ancestor_ids: Option<Set<Uuid>>,

    pub description: Option<Text>,

    #[serde(rename = "shortDescription")]
    pub short_description: Option<Text>,

    #[serde(rename = "descriptionMarkdown")]
    pub description_markdown: Option<Text>,

    #[serde(rename = "descriptionBase64")]
    pub description_base64: Option<Text>,

    #[serde(rename = "ownerId")]
    pub owner_id: Option<Uuid>,

    #[serde(rename = "ownerType")]
    pub profile_type: Option<Text>,

    #[serde(rename = "creatorId")]
    pub creator_id: Option<Uuid>,

    #[serde(rename = "editorIds")]
    pub editor_ids: Option<Set<Uuid>>,

    pub owner: Option<Frozen<Profile>>,

    #[serde(rename = "likesCount", default = "default_to_0")]
    pub like_count: Option<BigInt>,

    #[serde(rename = "coverImageKey")]
    pub cover_image_filename: Option<Text>,

    #[serde(rename = "coverImageURL")]
    pub cover_image_url: Option<Text>,

    #[serde(rename = "createdAt")]
    pub created_at: Option<Timestamp>,

    #[serde(rename = "updatedAt")]
    pub updated_at: Option<Timestamp>,

    #[charybdis(ignore)]
    #[serde(skip)]
    pub parent: Option<BaseNode>,

    #[charybdis(ignore)]
    #[serde(skip)]
    pub auth_branch: Option<AuthBranch>,

    #[charybdis(ignore)]
    #[serde(skip)]
    pub ctx: Context,
}

impl Node {
    pub async fn find_or_insert_branched(
        data: &crate::api::data::RequestData,
        id: Uuid,
        branch_id: Uuid,
        consistency: Option<Consistency>,
    ) -> Result<Self, NodecosmosError> {
        use charybdis::operations::InsertWithCallbacks;

        let pk = &(id, branch_id);
        let mut node_q = Self::maybe_find_by_primary_key_value(pk);

        if let Some(consistency) = consistency {
            node_q = node_q.consistency(consistency);
        }

        let node = node_q.execute(data.db_session()).await?;

        return match node {
            Some(node) => Ok(node),
            None => {
                let mut node = Self::find_by_primary_key_value(&(id, id))
                    .execute(data.db_session())
                    .await?;

                node.ctx = Context::BranchedInit;
                node.branch_id = branch_id;

                node.insert_cb(data).execute(data.db_session()).await?;

                Ok(node)
            }
        };
    }

    pub async fn find_by_ids_and_branch_id(
        db_session: &CachingSession,
        ids: &Set<Uuid>,
        branch_id: Uuid,
    ) -> Result<Vec<Node>, NodecosmosError> {
        let res = find_node!("id IN ? AND branch_id = ?", (ids, branch_id))
            .execute(db_session)
            .await?
            .try_collect()
            .await?;

        Ok(res)
    }
}

partial_node!(PkNode, id, branch_id, owner_id, editor_ids, ancestor_ids);

impl PkNode {
    pub async fn find_by_ids(db_session: &CachingSession, ids: &Vec<Uuid>) -> Result<Vec<PkNode>, NodecosmosError> {
        let res = find_pk_node!("id IN ? AND branch_id IN ?", (ids, ids))
            .execute(db_session)
            .await?
            .try_collect()
            .await?;

        Ok(res)
    }

    pub async fn find_by_ids_and_branch_id(
        db_session: &CachingSession,
        ids: &Vec<Uuid>,
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
    IndexNode,
    id,
    branch_id,
    owner_id,
    editor_ids,
    is_root,
    is_public,
    short_description,
    title,
    like_count,
    owner,
    cover_image_url,
    created_at,
    updated_at,
    parent_id,
    parent,
    auth_branch
);

partial_node!(
    BaseNode,
    root_id,
    id,
    branch_id,
    owner_id,
    editor_ids,
    is_root,
    short_description,
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
    GetDescriptionNode,
    id,
    branch_id,
    owner_id,
    editor_ids,
    is_public,
    description,
    description_markdown,
    cover_image_url,
    parent_id,
    parent,
    auth_branch,
    ctx
);

partial_node!(
    GetDescriptionBase64Node,
    id,
    branch_id,
    owner_id,
    editor_ids,
    is_public,
    description,
    description_markdown,
    description_base64,
    parent_id,
    parent,
    auth_branch,
    ctx
);

partial_node!(
    GetStructureNode,
    root_id,
    id,
    branch_id,
    owner_id,
    editor_ids,
    is_public,
    parent_id,
    ancestor_ids,
    order_index,
    parent,
    auth_branch,
    ctx
);

partial_node!(UpdateOrderNode, id, branch_id, parent_id, order_index);

partial_node!(UpdateLikesCountNode, id, branch_id, like_count, updated_at);

partial_node!(
    UpdateTitleNode,
    id,
    branch_id,
    order_index,
    owner_id,
    editor_ids,
    is_public,
    ancestor_ids,
    title,
    updated_at,
    ctx,
    root_id,
    parent_id,
    parent,
    auth_branch
);

partial_node!(
    UpdateDescriptionNode,
    id,
    branch_id,
    owner_id,
    editor_ids,
    is_public,
    description,
    short_description,
    description_markdown,
    description_base64,
    updated_at,
    ctx,
    parent_id,
    parent,
    auth_branch
);

impl UpdateDescriptionNode {
    pub fn from(node: Node) -> Self {
        Self {
            id: node.id,
            branch_id: node.branch_id,
            description: node.description,
            short_description: node.short_description,
            description_markdown: node.description_markdown,
            description_base64: node.description_base64,
            updated_at: node.updated_at,
            owner_id: node.owner_id,
            editor_ids: node.editor_ids,
            is_public: node.is_public,
            auth_branch: node.auth_branch,
            parent_id: node.parent_id,
            parent: node.parent,
            ctx: node.ctx,
        }
    }
}

partial_node!(
    UpdateCoverImageNode,
    id,
    branch_id,
    owner_id,
    editor_ids,
    is_public,
    cover_image_filename,
    cover_image_url,
    updated_at,
    parent_id,
    parent,
    auth_branch
);

partial_node!(PrimaryKeyNode, id, branch_id);

partial_node!(
    AuthNode,
    id,
    branch_id,
    owner_id,
    editor_ids,
    is_public,
    parent_id,
    parent,
    auth_branch
);

partial_node!(UpdateOwnerNode, id, branch_id, owner_id, owner, updated_at);
