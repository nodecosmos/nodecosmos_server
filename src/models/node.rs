mod auth;
pub mod callbacks;
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
use crate::models::branch::branchable::Branchable;
use crate::models::branch::AuthBranch;
use crate::models::node_descendant::NodeDescendant;
use crate::models::udts::Owner;
use crate::utils::defaults::default_to_0;
use crate::utils::defaults::default_to_false;
use charybdis::macros::charybdis_model;
use charybdis::operations::Find;
use charybdis::stream::CharybdisModelStream;
use charybdis::types::{BigInt, Boolean, Double, Frozen, Set, Text, Timestamp, Uuid};
use scylla::CachingSession;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Ancestors;

#[charybdis_model(
    table_name = nodes,
    partition_keys = [id],
    clustering_keys = [branch_id],
)]
#[derive(Serialize, Deserialize, Default, Clone)]
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

    #[serde(rename = "isPublic")]
    pub is_public: Boolean,

    #[serde(rename = "isRoot")]
    pub is_root: Boolean,

    #[serde(rename = "order")]
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
    pub owner_type: Option<Text>,

    #[serde(rename = "creatorId")]
    pub creator_id: Option<Uuid>,

    #[serde(rename = "editorIds")]
    pub editor_ids: Option<Set<Uuid>>,

    pub owner: Option<Frozen<Owner>>,

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
    pub merge: Boolean,
}

impl Node {
    pub async fn find_branch_nodes(
        db_session: &CachingSession,
        branch_id: Uuid,
        ids: &Set<Uuid>,
    ) -> Result<Vec<Node>, NodecosmosError> {
        let res = find_node!(db_session, "branch_id = ? AND id IN ?", (branch_id, ids))
            .await?
            .try_collect()
            .await?;

        Ok(res)
    }

    pub async fn init_auth_branch(&mut self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        let branch = AuthBranch::find_by_id(db_session, self.branch_id).await?;
        self.auth_branch = Some(branch);

        Ok(())
    }

    pub async fn parent(&mut self, db_session: &CachingSession) -> Result<Option<&mut BaseNode>, NodecosmosError> {
        if let (Some(parent_id), None) = (self.parent_id, &self.parent) {
            if self.is_branched() {
                return self.branch_parent(db_session).await;
            }

            let parent = BaseNode::find_by_primary_key_value(db_session, (parent_id, parent_id)).await?;

            self.parent = Some(parent);
        }

        Ok(self.parent.as_mut())
    }

    async fn branch_parent(&mut self, db_session: &CachingSession) -> Result<Option<&mut BaseNode>, NodecosmosError> {
        if let (Some(parent_id), None) = (self.parent_id, &self.parent) {
            let branch_parent = BaseNode::find_by_primary_key_value(db_session, (parent_id, self.branch_id))
                .await
                .ok();

            match branch_parent {
                Some(parent) => {
                    self.parent = Some(parent);
                }
                None => {
                    let parent = BaseNode::find_by_primary_key_value(db_session, (parent_id, parent_id)).await?;
                    self.parent = Some(parent);
                }
            }
        }

        Ok(self.parent.as_mut())
    }

    pub async fn transform_to_branched(&mut self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        let branch_self = Self::find_by_primary_key_value(db_session, (self.id, self.branch_id))
            .await
            .ok();

        match branch_self {
            Some(mut branch_self) => {
                branch_self.parent = self.parent.take();
                branch_self.auth_branch = self.auth_branch.take();

                *self = branch_self;
            }
            None => {
                let parent = self.parent.take();
                let auth_branch = self.auth_branch.take();
                let branch_id = self.branch_id;

                *self = Self::find_by_primary_key_value(db_session, (self.id, self.id)).await?;
                self.branch_id = branch_id;
                self.parent = parent;
                self.auth_branch = auth_branch;
            }
        }

        Ok(())
    }

    pub async fn descendants(
        &self,
        db_session: &CachingSession,
    ) -> Result<CharybdisModelStream<NodeDescendant>, NodecosmosError> {
        let descendants = NodeDescendant::find_by_root_id_and_branch_id_and_node_id(
            db_session,
            self.root_id,
            self.branch_id,
            self.id,
        )
        .await?;

        Ok(descendants)
    }

    pub async fn branch_descendants(
        &self,
        db_session: &CachingSession,
    ) -> Result<Vec<NodeDescendant>, NodecosmosError> {
        let main =
            NodeDescendant::find_by_root_id_and_branch_id_and_node_id(db_session, self.root_id, self.id, self.id)
                .await?
                .try_collect()
                .await?;

        let branched = NodeDescendant::find_by_root_id_and_branch_id_and_node_id(
            db_session,
            self.root_id,
            self.branch_id,
            self.id,
        )
        .await?
        .try_collect()
        .await?;

        let mut branched_ids = HashSet::with_capacity(branched.len());
        let mut descendants = Vec::with_capacity(main.len() + branched.len());

        for descendant in branched {
            branched_ids.insert(descendant.id);
            descendants.push(descendant);
        }

        for descendant in main {
            if !branched_ids.contains(&descendant.id) {
                descendants.push(descendant);
            }
        }

        descendants.sort_by(|a, b| {
            a.order_index
                .partial_cmp(&b.order_index)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        Ok(descendants)
    }
}

partial_node!(PkNode, id, branch_id);

impl PkNode {
    pub async fn find_and_collect_by_ids(
        db_session: &CachingSession,
        ids: &Vec<Uuid>,
    ) -> Result<Vec<PkNode>, NodecosmosError> {
        let res = find_pk_node!(db_session, "id IN ? AND branch_id IN ?", (ids, ids))
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
    is_root,
    is_public,
    short_description,
    title,
    like_count,
    owner,
    cover_image_url,
    created_at,
    updated_at
);

partial_node!(
    BaseNode,
    root_id,
    id,
    branch_id,
    is_root,
    short_description,
    ancestor_ids,
    order_index,
    title,
    parent_id,
    owner_id,
    editor_ids,
    like_count,
    owner,
    is_public,
    cover_image_url,
    created_at,
    updated_at
);

partial_node!(
    GetDescriptionNode,
    id,
    branch_id,
    description,
    description_markdown,
    cover_image_url
);

partial_node!(GetDescriptionBase64Node, id, branch_id, description_base64);

partial_node!(
    GetStructureNode,
    root_id,
    id,
    branch_id,
    parent_id,
    ancestor_ids,
    order_index
);

partial_node!(UpdateOrderNode, id, branch_id, parent_id, order_index);

partial_node!(UpdateLikesCountNode, id, branch_id, like_count, updated_at);

partial_node!(UpdateTitleNode, id, branch_id, title, updated_at);

partial_node!(
    UpdateDescriptionNode,
    id,
    branch_id,
    description,
    short_description,
    description_markdown,
    description_base64,
    updated_at
);

partial_node!(
    UpdateCoverImageNode,
    id,
    branch_id,
    owner_id,
    editor_ids,
    cover_image_filename,
    cover_image_url,
    updated_at
);

partial_node!(PrimaryKeyNode, id, branch_id);

partial_node!(AuthNode, id, branch_id, owner_id, editor_ids);

partial_node!(UpdateOwnerNode, id, branch_id, owner_id, owner, updated_at);

// TODO: this could be replaced with traits by utilizing AsNative from CharybdisModel
macro_rules! find_branched_or_original {
    ($struct_name:ident) => {
        impl $struct_name {
            #[allow(unused)]
            pub async fn find_branched_or_original(
                db_session: &CachingSession,
                id: Uuid,
                branch_id: Uuid,
            ) -> Result<Self, NodecosmosError> {
                let node = Self::find_by_primary_key_value(db_session, (id, branch_id))
                    .await
                    .ok();

                match node {
                    Some(node) => Ok(node),
                    None => Self::find_by_primary_key_value(db_session, (id, id))
                        .await
                        .map_err(|err| err.into()),
                }
            }

            #[allow(unused)]
            pub async fn find_branched(&mut self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
                let branch_self = Self::find_by_primary_key_value(db_session, (self.id, self.branch_id))
                    .await
                    .ok();

                match branch_self {
                    Some(branch_self) => {
                        *self = branch_self;
                    }
                    None => {
                        let branch_id = self.branch_id;

                        *self = Self::find_by_primary_key_value(db_session, (self.id, self.id)).await?;
                        self.branch_id = branch_id;
                    }
                }

                Ok(())
            }
        }
    };
}

find_branched_or_original!(Node);
find_branched_or_original!(GetStructureNode);
find_branched_or_original!(GetDescriptionNode);
find_branched_or_original!(GetDescriptionBase64Node);
