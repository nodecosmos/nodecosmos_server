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
use crate::models::node_descendant::NodeDescendant;
use crate::models::traits::Branchable;
use crate::models::udts::Profile;
use crate::utils::defaults::default_to_0;
use charybdis::macros::charybdis_model;
use charybdis::operations::Find;
use charybdis::stream::CharybdisModelStream;
use charybdis::types::{BigInt, Boolean, Double, Frozen, Set, Text, Timestamp, Uuid};
use scylla::statement::Consistency;
use scylla::CachingSession;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

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
        let mut node_q = Self::find_by_primary_key_value(pk);

        if let Some(consistency) = consistency {
            node_q = node_q.consistency(consistency);
        }

        let node = node_q.execute(data.db_session()).await;

        match node {
            Ok(node) => Ok(node),
            Err(e) => match e {
                charybdis::errors::CharybdisError::NotFoundError(_) => {
                    let mut node = Self::find_by_primary_key_value(&(id, id))
                        .execute(data.db_session())
                        .await?;

                    node.ctx = Context::BranchedInit;
                    node.branch_id = branch_id;

                    node.insert_cb(data).execute(data.db_session()).await?;

                    Ok(node)
                }
                _ => Err(e.into()),
            },
        }
    }

    pub async fn find_branch_nodes(
        db_session: &CachingSession,
        branch_id: Uuid,
        ids: &Set<Uuid>,
    ) -> Result<Vec<Node>, NodecosmosError> {
        let res = find_node!("branch_id = ? AND id IN ?", (branch_id, ids))
            .execute(db_session)
            .await?
            .try_collect()
            .await?;

        Ok(res)
    }

    pub async fn parent(&mut self, db_session: &CachingSession) -> Result<Option<&mut BaseNode>, NodecosmosError> {
        if let (Some(parent_id), None) = (self.parent_id, &self.parent) {
            if self.is_branched() {
                return self.branch_parent(db_session).await;
            }

            let parent = BaseNode::find_by_primary_key_value(&(parent_id, parent_id))
                .execute(db_session)
                .await?;

            self.parent = Some(parent);
        }

        Ok(self.parent.as_mut())
    }

    async fn branch_parent(&mut self, db_session: &CachingSession) -> Result<Option<&mut BaseNode>, NodecosmosError> {
        if let (Some(parent_id), None) = (self.parent_id, &self.parent) {
            let branch_parent = BaseNode::find_by_primary_key_value(&(parent_id, self.branch_id))
                .execute(db_session)
                .await
                .ok();

            match branch_parent {
                Some(parent) => {
                    self.parent = Some(parent);
                }
                None => {
                    let parent = BaseNode::find_by_primary_key_value(&(parent_id, parent_id))
                        .execute(db_session)
                        .await?;
                    self.parent = Some(parent);
                }
            }
        }

        Ok(self.parent.as_mut())
    }

    pub async fn transform_to_branched(&mut self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        let branch_self = Self::find_by_primary_key_value(&(self.id, self.branch_id))
            .execute(db_session)
            .await
            .ok();

        match branch_self {
            Some(mut branch_self) => {
                branch_self.parent = self.parent.take();
                branch_self.auth_branch = self.auth_branch.take();
                branch_self.ctx = self.ctx;

                *self = branch_self;
            }
            None => {
                let parent = self.parent.take();
                let auth_branch = self.auth_branch.take();
                let branch_id = self.branch_id;
                let ctx = self.ctx;

                *self = Self::find_by_primary_key_value(&(self.id, self.id))
                    .execute(db_session)
                    .await?;

                self.branch_id = branch_id;
                self.parent = parent;
                self.auth_branch = auth_branch;
                self.ctx = ctx;
            }
        }

        Ok(())
    }

    pub async fn descendants(
        &self,
        db_session: &CachingSession,
        consistency: Option<Consistency>,
    ) -> Result<CharybdisModelStream<NodeDescendant>, NodecosmosError> {
        let mut q = NodeDescendant::find_by_root_id_and_branch_id_and_node_id(self.root_id, self.branch_id, self.id);

        if let Some(consistency) = consistency {
            q = q.consistency(consistency)
        }

        let descendants = q.execute(db_session).await?;

        Ok(descendants)
    }

    pub async fn branch_descendants(
        &self,
        db_session: &CachingSession,
        consistency: Option<Consistency>,
    ) -> Result<Vec<NodeDescendant>, NodecosmosError> {
        let mut original_q = NodeDescendant::find_by_root_id_and_branch_id_and_node_id(self.root_id, self.id, self.id);
        let mut branched_q =
            NodeDescendant::find_by_root_id_and_branch_id_and_node_id(self.root_id, self.branch_id, self.id);

        if let Some(consistency) = consistency {
            original_q = original_q.consistency(consistency);
            branched_q = branched_q.consistency(consistency);
        }

        let original = original_q.execute(db_session).await?.try_collect().await?;
        let branched = branched_q.execute(db_session).await?.try_collect().await?;

        let mut branched_ids = HashSet::with_capacity(branched.len());
        let mut descendants = Vec::with_capacity(original.len() + branched.len());

        for descendant in branched {
            branched_ids.insert(descendant.id);
            descendants.push(descendant);
        }

        for descendant in original {
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

partial_node!(PkNode, id, branch_id, ancestor_ids);

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
    cover_image_url,
    ctx
);

partial_node!(
    GetDescriptionBase64Node,
    id,
    branch_id,
    description,
    description_markdown,
    description_base64,
    ctx
);

impl GetDescriptionBase64Node {}

partial_node!(
    GetStructureNode,
    root_id,
    id,
    branch_id,
    parent_id,
    ancestor_ids,
    order_index,
    ctx
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
    updated_at,
    ctx
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
    cover_image_filename,
    cover_image_url,
    updated_at
);

partial_node!(PrimaryKeyNode, id, branch_id);

partial_node!(AuthNode, id, branch_id, owner_id, editor_ids);

partial_node!(UpdateProfileNode, id, branch_id, owner_id, owner, updated_at);

macro_rules! find_branched_or_original {
    ($struct_name:ident) => {
        impl $struct_name {
            #[allow(unused)]
            pub async fn find_branched_or_original(
                db_session: &CachingSession,
                id: Uuid,
                branch_id: Uuid,
                consistency: Option<scylla::statement::Consistency>,
            ) -> Result<Self, NodecosmosError> {
                let pk = &(id, branch_id);
                let mut node_q = Self::find_by_primary_key_value(pk);

                if let Some(consistency) = consistency {
                    node_q = node_q.consistency(consistency);
                }

                let node = node_q.execute(db_session).await;

                match node {
                    Ok(node) => Ok(node),
                    Err(e) => match e {
                        charybdis::errors::CharybdisError::NotFoundError(_) => {
                            let mut node = Self::find_by_primary_key_value(&(id, id))
                                .execute(db_session)
                                .await?;

                            node.branch_id = branch_id;

                            Ok(node)
                        }
                        _ => Err(e.into()),
                    },
                }
            }

            #[allow(unused)]
            pub async fn find_branched(&mut self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
                let branch_self = Self::find_by_primary_key_value(&(self.id, self.branch_id))
                    .execute(db_session)
                    .await
                    .ok();

                match branch_self {
                    Some(branch_self) => {
                        *self = branch_self;
                    }
                    None => {
                        let branch_id = self.branch_id;

                        *self = Self::find_by_primary_key_value(&(self.id, self.id))
                            .execute(db_session)
                            .await?;
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
find_branched_or_original!(UpdateDescriptionNode);
