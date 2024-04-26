use serde::Deserialize;

#[derive(Deserialize, strum_macros::Display, Clone, Copy)]
pub enum ActionTypes {
    Create(ActionObject),
    Read(ActionObject),
    Update(ActionObject),
    Delete(ActionObject),
    Reorder(ActionObject),
    Merge,
}

#[derive(Deserialize, strum_macros::Display, Clone, Copy)]
pub enum ActionObject {
    Node,
    Workflow,
    Flow,
    FlowStep,
    Io,
    Comment,
}
