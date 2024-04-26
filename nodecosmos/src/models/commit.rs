use crate::models::udts::BranchReorderData;
use charybdis::macros::charybdis_model;
use charybdis::types::{Frozen, List, Map, Set, Text, Timestamp, Uuid};
use nodecosmos_macros::Branchable;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, strum_macros::Display, strum_macros::EnumString)]
pub enum CommitObject {
    /// For each ancestor we need to store node creation, deletion, title, description, and reordering
    CreateNode(Uuid),
    DeleteNodes(Vec<Uuid>),
    UndoDeleteNodes(Vec<Uuid>),
    RestoreNode(Uuid),
    EditNodeTitle(String),
    EditNodeDescription(String), // base64 encoded
    ReorderNode(BranchReorderData),
    CreatedWorkflowInitialInputs(Map<Uuid, Frozen<List<Uuid>>>),
    DeleteWorkflowInitialInputs(Map<Uuid, Frozen<List<Uuid>>>),
    CreateFlow(Uuid),
    DeleteFlow(Uuid),
    UndoDeleteFlow(Uuid),
    RestoreFlow(Uuid),
    EditFlowTitle(Uuid),
    EditFlowDescription(Uuid),
    CreateFlowStep(Uuid),
    DeleteFlowStep(Uuid),
    UndoDeleteFlowStep(Uuid),
    RestoreFlowStep(Uuid),
    KeepFlowStep(Uuid),
    CreatedFlowStepNodes(Map<Uuid, Frozen<Set<Uuid>>>),
    DeletedFlowStepNodes(Map<Uuid, Frozen<Set<Uuid>>>),
    CreatedFlowStepInputs(Map<Uuid, Frozen<Map<Uuid, Frozen<Set<Uuid>>>>>),
    DeletedFlowStepInputs(Map<Uuid, Frozen<Map<Uuid, Frozen<Set<Uuid>>>>>),
    CreatedFlowStepOutputs(Map<Uuid, Frozen<Map<Uuid, Frozen<Set<Uuid>>>>>),
    DeletedFlowStepOutputs(Map<Uuid, Frozen<Map<Uuid, Frozen<Set<Uuid>>>>>),
    EditFlowStepDescription(Uuid),
    CreateIo(Uuid),
    DeleteIo(Uuid),
    UndoDeleteIo(Uuid),
    RestoreIo(Uuid),
    EditIoTitle(Uuid),
    EditIoDescription(Uuid),
}

#[charybdis_model(
    table_name = commits,
    partition_keys = [node_id, branch_id],
    clustering_keys = [],
    global_secondary_indexes = []
)]
#[derive(Branchable, Serialize, Deserialize, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Commit {
    #[branch(original_id)]
    pub node_id: Uuid,

    pub branch_id: Uuid,

    pub object_id: Uuid,

    pub commit_object: Text,

    #[serde(default = "chrono::Utc::now")]
    pub created_at: Timestamp,
}
