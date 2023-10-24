use crate::actions::FlowStepCreationParams;
use crate::errors::NodecosmosError;
use crate::models::flow_step::BaseFlowStep;
use charybdis::operations::Find;
use scylla::CachingSession;

pub struct FlowStepIdxCalculator {
    prev_flow_step: Option<BaseFlowStep>,
    next_flow_step: Option<BaseFlowStep>,
}

impl FlowStepIdxCalculator {
    pub async fn new(db_session: &CachingSession, params: &FlowStepCreationParams) -> Result<Self, NodecosmosError> {
        let mut prev_flow_step = None;
        let mut next_flow_step = None;

        if let Some(flow_step) = &params.pref_flow_step {
            let fs = flow_step.find_by_primary_key(db_session).await?;
            prev_flow_step = Some(fs);
        }

        if let Some(flow_step) = &params.next_flow_step {
            let fs = flow_step.find_by_primary_key(db_session).await?;
            next_flow_step = Some(fs);
        }

        Ok(Self {
            prev_flow_step,
            next_flow_step,
        })
    }

    pub fn calculate_index(&self) -> f64 {
        if self.prev_flow_step.is_none() && self.next_flow_step.is_none() {
            return 0.0;
        }

        let prev_flow_step_index = if let Some(prev_flow_step) = &self.prev_flow_step {
            prev_flow_step.flow_index
        } else {
            0.0
        };

        let next_flow_step_index = if let Some(next_flow_step) = &self.next_flow_step {
            next_flow_step.flow_index
        } else {
            0.0
        };

        // If only the next flow step exists, return its flow index minus 1
        if self.prev_flow_step.is_none() {
            return next_flow_step_index - 1.0;
        }

        // If only the prev flow step exists, return its flow index plus 1
        if self.next_flow_step.is_none() {
            return prev_flow_step_index + 1.0;
        }

        // If both prev and next flow steps exist, return the average of their flow indices
        (prev_flow_step_index + next_flow_step_index) / 2.0
    }
}
