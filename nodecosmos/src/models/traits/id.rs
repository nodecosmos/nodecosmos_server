use charybdis::types::Uuid;

/// implemented by #[derive(Id)]
pub trait Id {
    fn id(&self) -> Uuid;
}

/// implemented by #[derive(RootId)]
pub trait RootId {
    #[allow(unused)]
    fn root_id(&self) -> Uuid;
}

/// implemented by #[derive(ObjectId)]
pub trait ObjectId {
    fn object_id(&self) -> Uuid;
}

/// implemented by #[derive(NodeId)]
pub trait NodeId {
    fn node_id(&self) -> Uuid;
}

/// implemented by #[derive(FlowId)]
pub trait FlowId {
    fn flow_id(&self) -> Uuid;
}

pub trait MaybeFlowId {
    fn maybe_flow_id(&self) -> Option<Uuid>;
}

pub trait FlowStepId {
    #[allow(unused)]
    fn flow_step_id(&self) -> Uuid;
}

pub trait MaybeFlowStepId {
    fn maybe_flow_step_id(&self) -> Option<Uuid>;
}
