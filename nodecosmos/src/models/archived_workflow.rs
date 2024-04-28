use crate::models::workflow::Workflow;
use charybdis::macros::charybdis_model;
use charybdis::types::{List, Text, Timestamp, Uuid};
use nodecosmos_macros::Branchable;
use serde::{Deserialize, Serialize};

#[charybdis_model(
    table_name = archived_workflows,
    partition_keys = [branch_id],
    clustering_keys = [node_id],
    global_secondary_indexes = []
)]
#[derive(Branchable, Serialize, Deserialize, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ArchivedWorkflow {
    #[branch(original_id)]
    pub node_id: Uuid,

    pub branch_id: Uuid,
    pub root_id: Uuid,
    pub title: Option<Text>,
    pub initial_input_ids: Option<List<Uuid>>,

    #[serde(default = "chrono::Utc::now")]
    pub created_at: Timestamp,

    #[serde(default = "chrono::Utc::now")]
    pub updated_at: Timestamp,
}

impl From<&Workflow> for ArchivedWorkflow {
    fn from(workflow: &Workflow) -> Self {
        Self {
            node_id: workflow.node_id,
            branch_id: workflow.branch_id,
            root_id: workflow.root_id,
            title: workflow.title.clone(),
            initial_input_ids: workflow.initial_input_ids.clone(),
            created_at: workflow.created_at,
            updated_at: workflow.updated_at,
        }
    }
}

partial_archived_workflow!(PkArchivedWorkflow, node_id, branch_id);
