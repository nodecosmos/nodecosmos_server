use std::fmt::Display;

#[allow(unused)]
pub enum ActionTypes {
    Create(ActionObject),
    Read(ActionObject),
    Update(ActionObject),
    Delete(ActionObject),
    Reorder(ActionObject),
}

impl Display for ActionTypes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ActionTypes::Create(action_object) => write!(f, "CREATE_{}", action_object),
            ActionTypes::Read(action_object) => write!(f, "READ_{}", action_object),
            ActionTypes::Update(action_object) => write!(f, "UPDATE_{}", action_object),
            ActionTypes::Delete(action_object) => write!(f, "DELETE_{}", action_object),
            ActionTypes::Reorder(action_object) => write!(f, "REORDER_{}", action_object),
        }
    }
}

#[allow(unused)]
pub enum ActionObject {
    Node,
    Workflow,
    Flow,
    FlowStep,
    InputOutput,
}

impl Display for ActionObject {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ActionObject::Node => write!(f, "NODE"),
            ActionObject::Workflow => write!(f, "WORKFLOW"),
            ActionObject::Flow => write!(f, "FLOW"),
            ActionObject::FlowStep => write!(f, "FLOW_STEP"),
            ActionObject::InputOutput => write!(f, "INPUT_OUTPUT"),
        }
    }
}
