pub mod callbacks;
/// TODO: consider moving all of this to node model
///       so we keep services dir only for generic use cases
pub mod cover_image_uploader;
mod create;
mod delete;
pub mod elastic_index;
pub mod reorder;
pub mod search;
mod update_description;
mod update_title;

use crate::errors::NodecosmosError;
use crate::models::node_descendant::NodeDescendant;
use crate::models::udts::{Creator, Owner};
use crate::utils::defaults::default_to_0;
use charybdis::macros::charybdis_model;
use charybdis::operations::Find;
use charybdis::stream::CharybdisModelStream;
use charybdis::types::{BigInt, Boolean, Double, Set, Text, Timestamp, Uuid};
use scylla::CachingSession;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[charybdis_model(
    table_name = nodes,
    partition_keys = [id],
    clustering_keys = [],
)]
#[derive(Serialize, Deserialize, Default, Clone)]
pub struct Node {
    #[serde(default)]
    pub id: Uuid,

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

    pub owner: Option<Owner>, // for front-end compatibility
    pub creator: Option<Creator>,

    #[serde(rename = "likesCount", default = "default_to_0")]
    pub likes_count: Option<BigInt>,

    #[serde(rename = "coverImageURL")]
    pub cover_image_url: Option<Text>,

    #[serde(rename = "coverImageKey")]
    pub cover_image_filename: Option<Text>,

    // timestamps
    #[serde(rename = "createdAt")]
    pub created_at: Option<Timestamp>,

    #[serde(rename = "updatedAt")]
    pub updated_at: Option<Timestamp>,

    #[charybdis(ignore)]
    #[serde(skip)]
    pub parent: Arc<Option<Node>>,
}

impl Node {
    pub async fn parent(&mut self, db_session: &CachingSession) -> Result<Arc<Option<Node>>, NodecosmosError> {
        if let Some(parent_id) = self.parent_id {
            if self.parent.is_none() {
                let parent = Self::find_by_primary_key_value(db_session, (parent_id,)).await?;

                self.parent = Arc::new(Some(parent));
            }
        }

        return Ok(Arc::clone(&self.parent));
    }

    pub async fn descendants(
        &self,
        db_session: &CachingSession,
    ) -> Result<CharybdisModelStream<NodeDescendant>, NodecosmosError> {
        let descendants = NodeDescendant::find_by_root_id_and_node_id(db_session, self.root_id, self.id).await?;

        Ok(descendants)
    }
}

partial_node!(
    BaseNode,
    root_id,
    id,
    is_root,
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
    GetDescriptionNode,
    root_id,
    id,
    description,
    description_markdown,
    cover_image_url
);

partial_node!(GetDescriptionBase64Node, id, description_base64);

partial_node!(GetStructureNode, root_id, id, parent_id, ancestor_ids, order_index);

partial_node!(UpdateOrderNode, id, parent_id, order_index);

partial_node!(UpdateLikesCountNode, id, likes_count, updated_at);

partial_node!(UpdateTitleNode, root_id, id, title, updated_at);

partial_node!(
    UpdateDescriptionNode,
    id,
    description,
    short_description,
    description_markdown,
    description_base64,
    updated_at
);

partial_node!(
    UpdateCoverImageNode,
    id,
    cover_image_url,
    cover_image_filename,
    updated_at
);

partial_node!(DeleteNode, id);
