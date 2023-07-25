use charybdis::{Map, Text, Uuid};
use charybdis_macros::{charybdis_model, partial_model_generator};

// pub enum CommitTypes {
//     Create(ObjectTypes),
//     Update(ObjectTypes),
//     Delete(ObjectTypes),
// }
//
// impl CommitTypes {
//     pub fn to_string(&self) -> String {
//         match self {
//             CommitTypes::Create(object_type) => format!("Create_{}", object_type.to_string()),
//             CommitTypes::Update(object_type) => format!("Update_{}", object_type.to_string()),
//             CommitTypes::Delete(object_type) => format!("Delete_{}", object_type.to_string()),
//         }
//     }
// }
//
// pub enum ObjectTypes {
//     Node,
//     Flow,
//     FlowStep,
//     InputOutput,
//     Workflow,
// }
//
// impl ObjectTypes {
//     pub fn to_string(&self) -> String {
//         match self {
//             ObjectTypes::Node => "Node".to_string(),
//             ObjectTypes::Flow => "Flow".to_string(),
//             ObjectTypes::FlowStep => "FlowStep".to_string(),
//             ObjectTypes::InputOutput => "InputOutput".to_string(),
//             ObjectTypes::Workflow => "Workflow".to_string(),
//         }
//     }
// }

#[partial_model_generator]
#[charybdis_model(
    table_name = commits,
    partition_keys = [contribution_request_id],
    clustering_keys = [id],
    secondary_indexes = []
)]
pub struct Commit {
    contribution_request_id: Uuid,
    id: Uuid,

    #[serde(rename = "commitType")]
    commit_type: Option<Text>,

    message: Option<Text>,
    details: Option<Map<Text, Text>>,
}
