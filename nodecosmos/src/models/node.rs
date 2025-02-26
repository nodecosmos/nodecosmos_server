use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::branch::AuthBranch;
use crate::models::node::delete::NodeDelete;
use crate::models::node_descendant::NodeDescendant;
use crate::models::subscription::Subscription;
use crate::models::traits::{
    AuthorizationFields, Branchable, ElasticDocument, FindBranchedOrOriginalNode, NodeBranchParams, WhereInChunksExec,
};
use crate::models::traits::{Context as Ctx, ModelContext};
use crate::models::udts::Profile;
use crate::stream::MergedModelStream;
use anyhow::Context;
use charybdis::batch::CharybdisModelBatch;
use charybdis::callbacks::Callbacks;
use charybdis::macros::charybdis_model;
use charybdis::model::AsNative;
use charybdis::types::{Boolean, Double, Frozen, Set, Text, Timestamp, Uuid};
use chrono::Utc;
use futures::StreamExt;
use macros::{Branchable, Id, NodeParent};
use scylla::CachingSession;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

mod auth;
mod create;
pub mod delete;
pub mod import;
pub mod reorder;
pub mod search;
pub mod sort;
mod subscription;
pub mod update_cover_image;
mod update_creator;
mod update_owner;
mod update_title;

#[charybdis_model(
    table_name = nodes,
    partition_keys = [branch_id],
    clustering_keys = [id],
)]
#[derive(Branchable, NodeParent, Id, Serialize, Deserialize, Default, Clone)]
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

    #[serde(default = "crate::models::utils::default_opt_false")]
    pub is_subscription_active: Option<Boolean>,

    #[serde(default)]
    pub is_root: Boolean,

    #[serde(default)]
    pub order_index: Double,

    #[serde(default)]
    pub owner_id: Uuid,

    #[serde(default)]
    pub creator_id: Option<Uuid>,

    pub title: Text,

    pub parent_id: Option<Uuid>,
    pub ancestor_ids: Option<Set<Uuid>>,
    pub owner: Option<Frozen<Profile>>,
    pub creator: Option<Frozen<Profile>>,
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
            self.set_defaults(data).await?;
            self.validate_root()?;
            self.validate_parent()?;
            self.validate_owner()?;
            self.update_branch_with_creation(data)
                .await
                .context("Failed to update branch")?;
        }

        self.create_workflow(data.db_session()).await?;
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
        // TODO: see nodecosmos/src/models/node/create.rs:258
        self.create_branched_if_original_exist(data).await?;

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

    pub async fn find_by_ids(db_session: &CachingSession, branch_id: Uuid, ids: &Vec<Uuid>) -> MergedModelStream<Node> {
        ids.where_in_chunked_query(db_session, |ids_chunk| {
            find_node!("branch_id = ? AND id IN ?", (branch_id, ids_chunk))
        })
        .await
    }
}

partial_node!(PkNode, branch_id, id, root_id, owner_id, editor_ids, ancestor_ids);

impl PkNode {
    pub async fn find_by_ids(
        db_session: &CachingSession,
        branch_id: Uuid,
        ids: &[Uuid],
    ) -> Result<Vec<PkNode>, NodecosmosError> {
        let nodes = ids
            .where_in_chunked_query(db_session, |ids_chunk| {
                find_pk_node!("branch_id = ? AND id IN ?", (branch_id, ids_chunk))
            })
            .await
            .try_collect()
            .await?;

        Ok(nodes)
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
    is_subscription_active,
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
    is_subscription_active,
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
    is_subscription_active,
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
            self.update_branch(data).await?;
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
            if self_clone.is_original() {
                let _ = self_clone.update_elastic_document(data.elastic_client()).await;
            }
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
    is_subscription_active,
    parent_id,
    parent,
    auth_branch
);

partial_node!(UpdateOwnerNode, branch_id, id, root_id, owner_id, owner, updated_at);
partial_node!(
    UpdateCreatorNode,
    branch_id,
    id,
    root_id,
    creator_id,
    creator,
    updated_at
);

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

        if self.is_original() {
            let _ = self.update_elastic_document(data.elastic_client()).await;
        }

        Ok(())
    }
}

partial_node!(UpdateEditorsNode, branch_id, id, root_id, editor_ids, updated_at);

impl UpdateEditorsNode {
    pub async fn find_by_ids(
        db_session: &CachingSession,
        branch_id: Uuid,
        ids: &[Uuid],
    ) -> Result<MergedModelStream<UpdateEditorsNode>, NodecosmosError> {
        Ok(ids
            .where_in_chunked_query(db_session, |ids_chunk| {
                find_update_editors_node!("branch_id = ? AND id IN ?", (branch_id, ids_chunk))
            })
            .await)
    }

    pub async fn update_editor_ids(
        data: &RequestData,
        node: &Node,
        added_editor_ids: &[Uuid],
        removed_editor_ids: &[Uuid],
    ) -> Result<(), NodecosmosError> {
        let root_id = node.root_id;
        let branch_id = node.branch_id;
        let node_id = node.id;

        let mut batch: CharybdisModelBatch<(Vec<Uuid>, Uuid, Uuid), Node> = CharybdisModelBatch::new();
        let mut descendants = NodeDescendant::find_by_root_id_and_branch_id_and_node_id(root_id, branch_id, node_id)
            .execute(data.db_session())
            .await?;
        let mut all_node_ids = vec![node_id];

        let mut append = |stmt: &str, node_id: Uuid, editor_ids: &[Uuid]| {
            for editor_id in editor_ids {
                batch.append_statement(stmt, (vec![*editor_id], branch_id, node_id));
            }
        };

        append(Node::PUSH_EDITOR_IDS_QUERY, node_id, added_editor_ids);
        append(Node::PULL_EDITOR_IDS_QUERY, node_id, removed_editor_ids);

        while let Some(descendant) = descendants.next().await {
            let descendant = descendant?;

            append(Node::PUSH_EDITOR_IDS_QUERY, descendant.id, added_editor_ids);
            append(Node::PULL_EDITOR_IDS_QUERY, descendant.id, removed_editor_ids);

            all_node_ids.push(descendant.id);
        }

        let mut nodes = UpdateEditorsNode::find_by_ids(data.db_session(), branch_id, &all_node_ids).await?;
        let mut update_editor_nodes: Vec<UpdateEditorsNode> = vec![];

        while let Some(node) = nodes.next().await {
            let node = node?;
            let mut editor_ids = node.editor_ids.unwrap_or(HashSet::new());

            for editor_id in added_editor_ids {
                editor_ids.insert(*editor_id);
            }

            for editor_id in removed_editor_ids {
                editor_ids.remove(editor_id);
            }

            update_editor_nodes.push(UpdateEditorsNode {
                id: node.id,
                branch_id: node.branch_id,
                editor_ids: Some(editor_ids),
                root_id: node.root_id,
                updated_at: Utc::now(),
            });
        }

        batch.execute(data.db_session()).await.map_err(|e| {
            log::error!(
                "Failed to update editor ids for node! RootId: {}, BranchId: {}, NodeId: {}, \nError: {:?}",
                root_id,
                branch_id,
                node_id,
                e
            );

            e
        })?;

        UpdateEditorsNode::bulk_update_elastic_documents(data.elastic_client(), &update_editor_nodes)
            .await
            .map_err(|e| {
                log::error!(
                "Failed to update editor ids for node in elastic! RootId: {}, BranchId: {}, NodeId: {}, \nError: {:?}",
                root_id,
                branch_id,
                node_id,
                e
            );

                e
            })?;

        if data.stripe_cfg().is_some() && !node.is_public() {
            // we can not remove members from subscription here, as users might be editor in other nodes.
            // owners have to remove editors from subscription manually on organization page.
            Subscription::update_members(data, node.root_id, added_editor_ids, &[])
                .await
                .map_err(|e| {
                    log::error!(
                        "FATAL: Failed to update subscription members! RootId: {}, BranchId: {}, NodeId: {}, \nError: {:?}",
                        root_id,
                        branch_id,
                        node_id,
                        e
                    );

                    e
                })?;
        }

        Ok(())
    }
}

partial_node!(FindCoverImageNode, branch_id, id, root_id, cover_image_url);

partial_node!(
    UpdateIsSubscriptionActiveNode,
    branch_id,
    id,
    root_id,
    is_subscription_active,
    updated_at
);
