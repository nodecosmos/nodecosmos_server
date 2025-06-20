use std::collections::HashSet;

use charybdis::model::BaseModel;
use charybdis::stream::CharybdisModelStream;
use charybdis::types::Uuid;
use futures::StreamExt;

use crate::errors::NodecosmosError;
use crate::models::traits::{FlowId, Id, MaybeFlowId, MaybeFlowStepId};
use crate::stream::MergedModelStream;

pub trait Pluck {
    fn pluck_id(&self) -> Vec<Uuid>;
    fn pluck_id_set(&self) -> HashSet<Uuid>;
}

impl<T: Id> Pluck for Vec<T> {
    fn pluck_id(&self) -> Vec<Uuid> {
        self.iter().map(|item| item.id()).collect()
    }

    fn pluck_id_set(&self) -> HashSet<Uuid> {
        self.iter().map(|item| item.id()).collect()
    }
}

impl<T: Id> Pluck for Option<Vec<T>> {
    fn pluck_id(&self) -> Vec<Uuid> {
        match self {
            Some(items) => items.iter().map(|item| item.id()).collect(),
            None => Vec::new(),
        }
    }

    fn pluck_id_set(&self) -> HashSet<Uuid> {
        match self {
            Some(items) => items.iter().map(|item| item.id()).collect(),
            None => HashSet::new(),
        }
    }
}

pub trait PluckFromStream {
    async fn pluck_id_set(&mut self) -> Result<HashSet<Uuid>, NodecosmosError>;
}

impl<T: Id + BaseModel> PluckFromStream for CharybdisModelStream<T> {
    async fn pluck_id_set(&mut self) -> Result<HashSet<Uuid>, NodecosmosError> {
        let mut ids = HashSet::new();

        while let Some(result) = self.next().await {
            match result {
                Ok(item) => {
                    ids.insert(item.id());
                }
                Err(e) => return Err(e.into()),
            }
        }

        Ok(ids)
    }
}

impl<T: Id + BaseModel> PluckFromStream for MergedModelStream<T> {
    async fn pluck_id_set(&mut self) -> Result<HashSet<Uuid>, NodecosmosError> {
        let mut ids = HashSet::new();

        while let Some(result) = self.next().await {
            match result {
                Ok(item) => {
                    ids.insert(item.id());
                }
                Err(e) => return Err(e.into()),
            }
        }

        Ok(ids)
    }
}

pub trait PluckFlowId {
    fn pluck_flow_id(&self) -> HashSet<Uuid>;
}

impl<T: FlowId> PluckFlowId for Vec<T> {
    fn pluck_flow_id(&self) -> HashSet<Uuid> {
        self.iter().map(|item| item.flow_id()).collect()
    }
}

impl<T: FlowId> PluckFlowId for Option<Vec<T>> {
    fn pluck_flow_id(&self) -> HashSet<Uuid> {
        match self {
            Some(items) => items.iter().map(|item| item.flow_id()).collect(),
            None => HashSet::new(),
        }
    }
}

pub trait MaybePluckFlowId {
    fn maybe_pluck_flow_id(&self) -> HashSet<Uuid>;
}

impl<T: MaybeFlowId> MaybePluckFlowId for Vec<T> {
    fn maybe_pluck_flow_id(&self) -> HashSet<Uuid> {
        self.iter().filter_map(|item| item.maybe_flow_id()).collect()
    }
}

impl<T: MaybeFlowId> MaybePluckFlowId for Option<Vec<T>> {
    fn maybe_pluck_flow_id(&self) -> HashSet<Uuid> {
        match self {
            Some(items) => items.iter().filter_map(|item| item.maybe_flow_id()).collect(),
            None => HashSet::new(),
        }
    }
}

pub trait MaybePluckFlowStepId {
    fn maybe_pluck_flow_step_id(&self) -> HashSet<Uuid>;
}

impl<T: MaybeFlowStepId> MaybePluckFlowStepId for Vec<T> {
    fn maybe_pluck_flow_step_id(&self) -> HashSet<Uuid> {
        self.iter().filter_map(|item| item.maybe_flow_step_id()).collect()
    }
}

impl<T: MaybeFlowStepId> MaybePluckFlowStepId for Option<Vec<T>> {
    fn maybe_pluck_flow_step_id(&self) -> HashSet<Uuid> {
        match self {
            Some(items) => items.iter().filter_map(|item| item.maybe_flow_step_id()).collect(),
            None => HashSet::new(),
        }
    }
}
