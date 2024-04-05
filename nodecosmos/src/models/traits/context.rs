use crate::models::flow::{Flow, UpdateTitleFlow};
use crate::models::flow_step::{
    FlowStep, SiblingFlowStep, UpdateInputIdsFlowStep, UpdateNodeIdsFlowStep, UpdateOutputIdsFlowStep,
};
use crate::models::input_output::{Io, UpdateTitleIo};
use crate::models::node::{Node, UpdateTitleNode};
use crate::models::workflow::Workflow;

#[derive(PartialEq, Default, Clone, Copy)]
pub enum Context {
    #[default]
    None,
    Merge,
    BranchedInit,
    ParentDelete,
}

pub trait ModelContext {
    fn context(&mut self) -> &mut Context;
    fn context_ref(&self) -> &Context;

    fn set_merge_context(&mut self) {
        *self.context() = Context::Merge;
    }

    fn set_branched_init_context(&mut self) {
        *self.context() = Context::BranchedInit;
    }

    fn set_parent_delete_context(&mut self) {
        *self.context() = Context::ParentDelete;
    }

    fn is_default_context(&self) -> bool {
        self.context_ref() == &Context::None
    }

    fn is_merge_context(&self) -> bool {
        self.context_ref() == &Context::Merge
    }

    fn is_branched_init_context(&self) -> bool {
        self.context_ref() == &Context::BranchedInit
    }

    fn is_parent_delete_context(&self) -> bool {
        self.context_ref() == &Context::ParentDelete
    }
}

macro_rules! impl_context {
    ($($t:ty),*) => {
        $(
            impl crate::models::traits::ModelContext for $t {
                fn context(&mut self) -> &mut crate::models::traits::Context {
                    &mut self.ctx
                }

                  fn context_ref(&self) -> &crate::models::traits::Context {
                    &self.ctx
                }
            }
        )*
    };
}

impl_context!(
    Node,
    UpdateTitleNode,
    Workflow,
    Flow,
    UpdateTitleFlow,
    FlowStep,
    SiblingFlowStep,
    UpdateNodeIdsFlowStep,
    UpdateInputIdsFlowStep,
    UpdateOutputIdsFlowStep,
    Io,
    UpdateTitleIo
);
