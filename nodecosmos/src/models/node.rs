mod create_node;
mod delete_node;
mod node_tree_handler;

use crate::actions::client_session::CurrentUser;
use crate::app::CbExtension;
use crate::errors::NodecosmosError;
use crate::models::helpers::{
    default_to_0, default_to_false, impl_node_updated_at_with_elastic_ext_cb, impl_updated_at_cb,
    sanitize_description_ext_cb_fn,
};
use crate::models::node_descendant::NodeDescendant;
use crate::models::udts::{Creator, Owner, OwnerTypes};
use crate::services::elastic::update_elastic_document;
use charybdis::*;
use chrono::Utc;

#[partial_model_generator]
#[charybdis_model(
    table_name = nodes,
    partition_keys = [id],
    clustering_keys = [],
    secondary_indexes = [],
)]
pub struct Node {
    #[serde(default = "Uuid::new_v4")]
    pub id: Uuid,

    #[serde(default, rename = "rootId")]
    pub root_id: Uuid,

    #[serde(rename = "isPublic")]
    pub is_public: Option<Boolean>,

    #[serde(rename = "isRoot", default = "default_to_false")]
    pub is_root: Option<Boolean>,

    #[serde(rename = "parentId")]
    pub parent_id: Option<Uuid>,

    #[serde(rename = "ancestorIds")]
    pub ancestor_ids: Option<List<Uuid>>,

    // node
    pub title: Option<Text>,
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

    pub owner: Option<Owner>, // for front-end compatibility
    pub creator: Option<Creator>,

    // timestamps
    #[serde(rename = "createdAt")]
    pub created_at: Option<Timestamp>,

    #[serde(rename = "updatedAt")]
    pub updated_at: Option<Timestamp>,

    #[serde(rename = "likesCount", default = "default_to_0")]
    pub likes_count: Option<BigInt>,

    #[serde(rename = "coverImageUrl")]
    pub cover_image_url: Option<Text>,

    #[serde(rename = "coverImageKey")]
    pub cover_image_filename: Option<Text>,

    #[serde(rename = "order")]
    pub order_index: Option<Double>,
}

impl Node {
    pub const ELASTIC_IDX_NAME: &'static str = "nodes";

    pub async fn parent(
        &self,
        db_session: &CachingSession,
    ) -> Result<Option<Node>, NodecosmosError> {
        if let Some(parent_id) = self.parent_id {
            let node = Self {
                id: parent_id,
                ..Default::default()
            }
            .find_by_primary_key(db_session)
            .await?;

            Ok(Some(node))
        } else {
            Ok(None)
        }
    }

    pub async fn descendants(
        &self,
        db_session: &CachingSession,
    ) -> Result<Vec<NodeDescendant>, CharybdisError> {
        let descendants =
            NodeDescendant::find_by_root_id_and_node_id(db_session, self.root_id, self.id).await?;

        Ok(descendants)
    }

    pub fn set_owner(&mut self, current_user: CurrentUser) {
        let owner = Owner {
            id: current_user.id,
            name: current_user.full_name(),
            username: Some(current_user.username),
            owner_type: OwnerTypes::User.into(),
            profile_image_url: None,
        };

        self.owner_id = Some(owner.id);
        self.owner_type = Some(owner.owner_type.clone());

        self.owner = Some(owner);
    }

    pub async fn set_defaults(&mut self, parent: Option<Self>) -> Result<(), CharybdisError> {
        if let Some(parent) = parent {
            self.root_id = parent.root_id;
            self.editor_ids = parent.editor_ids;
            self.is_public = parent.is_public;

            let mut ancestor_ids = parent.ancestor_ids.unwrap_or_default();
            ancestor_ids.push(parent.id);

            self.ancestor_ids = Some(ancestor_ids);
        } else {
            self.is_root = Some(true);
            self.root_id = self.id;
        }

        let now = Utc::now();

        self.created_at = Some(now);
        self.updated_at = Some(now);

        Ok(())
    }
}

impl ExtCallbacks<CbExtension> for Node {
    async fn after_insert(
        &mut self,
        db_session: &CachingSession,
        ext: &CbExtension,
    ) -> Result<(), CharybdisError> {
        self.append_to_ancestors(db_session).await?;
        self.add_to_elastic_index(ext).await;

        Ok(())
    }

    async fn before_delete(
        &mut self,
        db_session: &CachingSession,
        ext: &CbExtension,
    ) -> Result<(), CharybdisError> {
        self.delete_related_data(db_session, ext).await?;

        Ok(())
    }
}

//----------------------------------------------------------------------------------------------------------------------
partial_node!(
    BaseNode,
    root_id,
    id,
    short_description,
    ancestor_ids,
    order_index,
    title,
    parent_id,
    owner_id,
    editor_ids,
    likes_count,
    owner,
    is_public,
    cover_image_url,
    created_at,
    updated_at
);

partial_node!(
    GetNodeDescription,
    root_id,
    id,
    description,
    description_markdown
);

partial_node!(GetNodedescriptionBase64, id, description_base64);

partial_node!(
    ReorderNode,
    root_id,
    id,
    parent_id,
    ancestor_ids,
    order_index
);

partial_node!(UpdateNodeAncestorIds, id, parent_id, ancestor_ids);

partial_node!(UpdateNodeOrder, id, parent_id, order_index);
//----------------------------------------------------------------------------------------------------------------------
partial_node!(
    UpdateNodeTitle,
    root_id,
    parent_id,
    id,
    title,
    ancestor_ids,
    order_index,
    updated_at
);

impl UpdateNodeTitle {
    pub fn set_defaults(&mut self, native_node: Node) {
        self.updated_at = Some(Utc::now());

        self.root_id = native_node.root_id;
        self.parent_id = native_node.parent_id;
        self.order_index = native_node.order_index;
        self.ancestor_ids = native_node.ancestor_ids;
    }
}

impl ExtCallbacks<CbExtension> for UpdateNodeTitle {
    async fn after_update(
        &mut self,
        session: &CachingSession,
        ext: &CbExtension,
    ) -> Result<(), CharybdisError> {
        self.update_node_descendants(session).await?;

        update_elastic_document(
            &ext.elastic_client,
            Node::ELASTIC_IDX_NAME,
            self,
            self.id.to_string(),
        )
        .await;

        Ok(())
    }
}

//----------------------------------------------------------------------------------------------------------------------
partial_node!(
    UpdateNodeDescription,
    root_id,
    id,
    description,
    short_description,
    description_markdown,
    description_base64,
    updated_at
);
impl ExtCallbacks<CbExtension> for UpdateNodeDescription {
    sanitize_description_ext_cb_fn!();

    // TODO: introduce bounce queue
    async fn after_update(
        &mut self,
        _db_session: &CachingSession,
        ext: &CbExtension,
    ) -> Result<(), CharybdisError> {
        update_elastic_document(
            &ext.elastic_client,
            Node::ELASTIC_IDX_NAME,
            self,
            self.id.to_string(),
        )
        .await;

        Ok(())
    }
}

partial_node!(UpdateNodeOwner, id, owner_id, updated_at);
impl_updated_at_cb!(UpdateNodeOwner);

partial_node!(UpdateNodeLikesCount, id, likes_count, updated_at);
impl_node_updated_at_with_elastic_ext_cb!(UpdateNodeLikesCount);

partial_node!(
    UpdateNodeCoverImage,
    id,
    cover_image_url,
    cover_image_filename,
    updated_at
);
impl_node_updated_at_with_elastic_ext_cb!(UpdateNodeCoverImage);

partial_node!(DeleteNode, id);
