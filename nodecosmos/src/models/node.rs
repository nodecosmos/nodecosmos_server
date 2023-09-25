mod create_node;
mod delete_node;

use crate::app::CbExtension;
use crate::errors::NodecosmosError;
use crate::models::helpers::{
    default_to_0, default_to_false, impl_node_updated_at_with_elastic_ext_cb, impl_updated_at_cb,
    sanitize_description_ext_cb_fn,
};
use crate::models::udts::{Creator, Owner};
use crate::services::elastic::update_elastic_document;
use charybdis::*;
use chrono::Utc;

#[partial_model_generator]
#[charybdis_model(
    table_name = nodes,
    partition_keys = [root_id],
    clustering_keys = [id],
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

    #[serde(rename = "childIds")]
    pub child_ids: Option<List<Uuid>>,

    #[serde(rename = "descendantIds")]
    pub descendant_ids: Option<List<Uuid>>,

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
}

impl Node {
    pub const ELASTIC_IDX_NAME: &'static str = "nodes";

    pub async fn parent(
        &self,
        db_session: &CachingSession,
    ) -> Result<Option<Node>, NodecosmosError> {
        match self.parent_id {
            Some(parent_id) => {
                let mut parent = Node::new();
                parent.id = parent_id;
                parent.root_id = self.root_id;

                let parent = parent.find_by_primary_key(db_session).await?;
                Ok(Some(parent))
            }
            None => Ok(None),
        }
    }

    pub fn set_owner(&mut self, owner: Owner) {
        self.owner_id = Some(owner.id);
        self.owner_type = Some(owner.owner_type.clone());

        self.owner = Some(owner);
    }

    pub async fn set_defaults(&mut self) -> Result<(), CharybdisError> {
        if self.root_id == Uuid::nil() {
            self.root_id = self.id;
        }

        self.created_at = Some(Utc::now());
        self.updated_at = Some(Utc::now());
        self.is_root = Some(self.parent_id.is_none());

        Ok(())
    }
}

impl ExtCallbacks<CbExtension> for Node {
    async fn before_insert(
        &mut self,
        _db_session: &CachingSession,
        _ext: &CbExtension,
    ) -> Result<(), CharybdisError> {
        self.set_defaults().await?;

        Ok(())
    }

    async fn after_insert(
        &mut self,
        db_session: &CachingSession,
        ext: &CbExtension,
    ) -> Result<(), CharybdisError> {
        self.add_related_data(db_session).await?;
        self.add_to_elastic_index(ext).await;

        Ok(())
    }

    async fn before_delete(
        &mut self,
        db_session: &CachingSession,
        _extension: &CbExtension,
    ) -> Result<(), CharybdisError> {
        self.delete_related_data(db_session).await?;

        Ok(())
    }

    async fn after_delete(
        &mut self,
        _db_session: &CachingSession,
        ext: &CbExtension,
    ) -> Result<(), CharybdisError> {
        self.delete_related_elastic_data(ext).await;

        Ok(())
    }
}

partial_node!(
    BaseNode,
    root_id,
    id,
    short_description,
    ancestor_ids,
    child_ids,
    descendant_ids,
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

partial_node!(GetNodedescriptionBase64, root_id, id, description_base64);

partial_node!(
    ReorderNode,
    root_id,
    id,
    parent_id,
    child_ids,
    ancestor_ids,
    descendant_ids
);

partial_node!(UpdateParent, root_id, id, parent_id);
partial_node!(UpdateAncestors, root_id, id, ancestor_ids);
partial_node!(UpdateChildIds, root_id, id, child_ids);
partial_node!(UpdateDescendantIds, root_id, id, descendant_ids);

partial_node!(UpdateNodeTitle, root_id, id, title, updated_at);
impl_node_updated_at_with_elastic_ext_cb!(UpdateNodeTitle);

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

partial_node!(UpdateNodeOwner, root_id, id, owner_id, updated_at);
impl_updated_at_cb!(UpdateNodeOwner);

partial_node!(UpdateNodeLikesCount, root_id, id, likes_count, updated_at);
impl_node_updated_at_with_elastic_ext_cb!(UpdateNodeLikesCount);

partial_node!(
    UpdateNodeCoverImage,
    root_id,
    id,
    cover_image_url,
    cover_image_filename,
    updated_at
);
impl_node_updated_at_with_elastic_ext_cb!(UpdateNodeCoverImage);

partial_node!(DeleteNode, root_id, id);
