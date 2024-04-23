use crate::models::description::Description;
use charybdis::macros::charybdis_model;
use charybdis::types::{Text, Timestamp, Uuid};
use nodecosmos_macros::{Branchable, ObjectId};
use serde::{Deserialize, Serialize};

#[charybdis_model(
    table_name = archived_descriptions,
    partition_keys = [object_id],
    clustering_keys = [branch_id]
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

impl From<Description> for ArchivedDescription {
    fn from(description: Description) -> Self {
        Self {
            object_id: description.object_id,
            branch_id: description.branch_id,
            node_id: description.node_id,
            root_id: description.root_id,
            object_type: description.object_type,
            short_description: description.short_description,
            html: description.html,
            markdown: description.markdown,
            base64: description.base64,
            updated_at: description.updated_at,
        }
    }
}
