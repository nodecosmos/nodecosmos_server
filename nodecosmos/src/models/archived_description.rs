use crate::models::description::Description;
use charybdis::macros::charybdis_model;
use charybdis::types::{Text, Timestamp, Uuid};
use nodecosmos_macros::{Branchable, ObjectId};
use serde::{Deserialize, Serialize};

#[charybdis_model(
    table_name = archived_descriptions,
    partition_keys = [object_id],
    clustering_keys = [branch_id],
    table_options = r#"
        compression = {
            'sstable_compression': 'SnappyCompressor',
            'chunk_length_in_kb': 64
        }
    "#
)]
#[derive(Default, Clone, Branchable, ObjectId, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchivedDescription {
    pub object_id: Uuid,
    pub branch_id: Uuid,

    #[branch(original_id)]
    pub node_id: Uuid,

    #[serde(default)]
    pub root_id: Uuid,

    pub object_type: Text,
    pub short_description: Option<Text>,
    pub html: Option<Text>,
    pub markdown: Option<Text>,
    pub base64: Option<Text>,

    #[serde(default = "chrono::Utc::now")]
    pub updated_at: Timestamp,
}

impl From<&Description> for ArchivedDescription {
    fn from(description: &Description) -> Self {
        Self {
            object_id: description.object_id,
            branch_id: description.branch_id,
            node_id: description.node_id,
            root_id: description.root_id,
            object_type: description.object_type.clone(),
            short_description: description.short_description.clone(),
            html: description.html.clone(),
            markdown: description.markdown.clone(),
            base64: description.base64.clone(),
            updated_at: description.updated_at,
        }
    }
}

partial_archived_description!(PkArchivedDescription, object_id, branch_id, node_id);

impl From<&Description> for PkArchivedDescription {
    fn from(description: &Description) -> Self {
        Self {
            object_id: description.object_id,
            branch_id: description.branch_id,
            node_id: description.node_id,
        }
    }
}
