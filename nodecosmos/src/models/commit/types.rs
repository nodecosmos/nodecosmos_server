use std::fmt::Display;

pub enum CommitTypes {
    Create(CommitObjectTypes),
    Update(CommitObjectTypes),
    Delete(CommitObjectTypes),
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

pub enum CommitObjectTypes {
    Node(Committable),
    // Flow,
    // FlowStep,
    // Io,
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

impl Display for CommitObjectTypes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CommitObjectTypes::Node(committable) => write!(f, "NODE_{}", committable),
            // CommitObjectTypes::Flow => write!(f, "Flow"),
            // CommitObjectTypes::FlowStep => write!(f, "FlowStep"),
            // CommitObjectTypes::Io => write!(f, "Io"),
            CommitObjectTypes::Workflow(committable) => write!(f, "WORKFLOW_{}", committable),
        }
    }
}
