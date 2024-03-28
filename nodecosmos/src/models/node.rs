mod auth;
pub mod context;
mod create;
mod delete;
pub mod reorder;
pub mod search;
pub mod sort;
pub mod update_cover_image;
mod update_owner;
mod update_title;

use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::branch::AuthBranch;
use crate::models::node::context::Context;
use crate::models::traits::Branchable;
use crate::models::udts::Profile;
use charybdis::callbacks::Callbacks;
use charybdis::macros::charybdis_model;
use charybdis::model::AsNative;
use charybdis::operations::Find;
use charybdis::types::{Boolean, Double, Frozen, Int, Set, Text, Timestamp, Uuid};
use futures::TryFutureExt;
use log::error;
use nodecosmos_macros::{Branchable, Id, NodeAuthorization, NodeParent};
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
#[derive(Branchable, NodeParent, NodeAuthorization, Id, Serialize, Deserialize, Default, Clone)]
pub struct Node {
    #[serde(default)]
    #[branch(original_id)]
    pub id: Uuid,

    // `self.branch_id` is equal to `self.id` for the main node's branch
    #[serde(default, rename = "branchId")]
    pub branch_id: Uuid,

    #[serde(default, rename = "rootId")]
    pub root_id: Uuid,

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

    #[serde(rename = "ownerId")]
    pub owner_id: Option<Uuid>,

    pub owner: Option<Frozen<Profile>>,

    #[serde(rename = "editorIds")]
    pub editor_ids: Option<Set<Uuid>>,

    #[serde(rename = "likesCount", default)]
    pub like_count: Int,

    #[serde(rename = "coverImageKey")]
    pub cover_image_filename: Option<Text>,

    #[serde(rename = "coverImageURL")]
    pub cover_image_url: Option<Text>,

    #[serde(rename = "createdAt", default = "chrono::Utc::now")]
    pub created_at: Timestamp,

    #[serde(rename = "updatedAt", default = "chrono::Utc::now")]
    pub updated_at: Timestamp,

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

        if self.ctx != Context::BranchedInit {
            if let Err(e) = self.update_branch_with_creation(data).await {
                self.remove_from_ancestors(db_session).await?;
                error!("[before_insert] Unexpected error updating branch with creation: {}", e);
            }
        }

        Ok(())
    }

    async fn after_insert(&mut self, _: &CachingSession, data: &Self::Extension) -> Result<(), NodecosmosError> {
        let self_clone = self.clone();
        let data = data.clone();

        tokio::spawn(async move {
            self_clone.add_to_elastic(data.elastic_client()).await;
            self_clone.create_new_version(&data).await;
            self_clone.create_workflow(&data).await;
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

impl Node {
    pub async fn find_or_insert_branched(
        data: &RequestData,
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

                node.insert_cb(data)
                    .execute(data.db_session())
                    .map_err(|err| NodecosmosError::from(err))
                    .await?;

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
    BaseNode,
    root_id,
    id,
    branch_id,
    owner_id,
    editor_ids,
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

impl Callbacks for UpdateTitleNode {
    type Extension = RequestData;
    type Error = NodecosmosError;

    async fn before_update(&mut self, _: &CachingSession, data: &Self::Extension) -> Result<(), NodecosmosError> {
        if self.is_branched() {
            self.as_native().create_branched_if_not_exist(data).await?;
        }

        self.update_branch(&data).await?;

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

partial_node!(UpdateLikesCountNode, id, branch_id, like_count, updated_at);

partial_node!(
    UpdateCoverImageNode,
    id,
    branch_id,
    cover_image_filename,
    cover_image_url,
    updated_at
);

macro_rules! impl_node_updated_at_with_elastic_ext_cb {
    ($struct_name:ident) => {
        impl charybdis::callbacks::Callbacks for $struct_name {
            type Extension = crate::api::data::RequestData;
            type Error = crate::errors::NodecosmosError;

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
