use crate::models::flow::{Flow, UpdateTitleFlow};
use crate::models::flow_step::FlowStep;
use crate::models::input_output::{Io, UpdateTitleIo};
use crate::models::node::{Node, UpdateTitleNode};
use crate::models::workflow::Workflow;

#[derive(PartialEq, Default, Clone)]
pub enum Context {
    #[default]
    None,
    Merge,
    BranchedInit,
}

impl Context {
    pub fn merge(&mut self) {
        *self = Context::Merge;
    }

    pub fn branched_init(&mut self) {
        *self = Context::BranchedInit;
    }

    pub fn is_default(&self) -> bool {
        *self == Context::None
    }

    pub fn is_merge(&self) -> bool {
        *self == Context::Merge
    }

    pub fn is_branched_init(&self) -> bool {
        *self == Context::BranchedInit
    }
}

pub trait ModelContext {
    fn context(&mut self) -> &mut Context;
    fn context_ref(&self) -> &Context;

    fn set_merge_context(&mut self) {
        self.context().merge();
    }

    fn set_branched_init_context(&mut self) {
        self.context().branched_init();
    }

    fn is_default_context(&self) -> bool {
        self.context_ref().is_default()
    }

    fn is_merge_context(&self) -> bool {
        self.context_ref().is_merge()
    }

    fn is_branched_init_context(&self) -> bool {
        self.context_ref().is_branched_init()
    }
}

macro_rules! impl_context {
    ($($t:ty),*) => {
        $(
            impl crate::models::traits::context::ModelContext for $t {
                fn context(&mut self) -> &mut crate::models::traits::context::Context {
                    &mut self.ctx
                }

                  fn context_ref(&self) -> &crate::models::traits::context::Context {
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
    Io,
    UpdateTitleIo
);
