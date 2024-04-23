use crate::models::node::Node;
use crate::models::udts::Profile;
use charybdis::macros::charybdis_model;
use charybdis::types::{Boolean, Double, Frozen, Set, Text, Timestamp, Uuid};
use nodecosmos_macros::{Branchable, Id, NodeAuthorization, NodeParent};
use serde::{Deserialize, Serialize};

#[charybdis_model(
    table_name = archived_nodes,
    partition_keys = [id],
    clustering_keys = [branch_id],
    table_options = r#"
        compression = {
            'sstable_compression': 'SnappyCompressor',
            'chunk_length_in_kb': 64
        }
    "#
)]
#[derive(Branchable, NodeParent, NodeAuthorization, Id, Serialize, Deserialize, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ArchivedNode {
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
}

impl From<Node> for ArchivedNode {
    fn from(node: Node) -> Self {
        ArchivedNode {
            branch_id: node.branch_id,
            id: node.id,
            root_id: node.root_id,
            is_public: node.is_public,
            is_root: node.is_root,
            order_index: node.order_index,
            title: node.title,
            parent_id: node.parent_id,
            ancestor_ids: node.ancestor_ids,
            owner_id: node.owner_id,
            owner: node.owner,
            editor_ids: node.editor_ids,
            viewer_ids: node.viewer_ids,
            cover_image_filename: node.cover_image_filename,
            cover_image_url: node.cover_image_url,
            created_at: node.created_at,
            updated_at: node.updated_at,
        }
    }
}
