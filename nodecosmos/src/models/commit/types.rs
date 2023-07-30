use std::fmt::Display;

pub enum CommitTypes {
    Create(ObjectTypes),
    Update(ObjectTypes),
    Delete(ObjectTypes),
}

impl Display for CommitTypes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CommitTypes::Create(object_type) => write!(f, "CREATE_{}", object_type),
            CommitTypes::Update(object_type) => write!(f, "UPDATE_{}", object_type),
            CommitTypes::Delete(object_type) => write!(f, "DELETE_{}", object_type),
        }
    }
}

pub enum ObjectTypes {
    Node(Committable),
    // Flow,
    // FlowStep,
    // InputOutput,
    Workflow(Committable),
}

pub enum Committable {
    BaseObject,
    Title,
    Description,
}

impl Display for Committable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Committable::BaseObject => write!(f, "BASE"),
            Committable::Title => write!(f, "TITLE"),
            Committable::Description => write!(f, "DESCRIPTION"),
        }
    }
}

impl Display for ObjectTypes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ObjectTypes::Node(committable) => write!(f, "NODE_{}", committable),
            // ObjectTypes::Flow => write!(f, "Flow"),
            // ObjectTypes::FlowStep => write!(f, "FlowStep"),
            // ObjectTypes::InputOutput => write!(f, "InputOutput"),
            ObjectTypes::Workflow(committable) => write!(f, "WORKFLOW_{}", committable),
        }
    }
}
