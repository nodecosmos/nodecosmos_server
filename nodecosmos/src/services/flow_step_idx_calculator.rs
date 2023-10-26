use crate::actions::FlowStepCreationParams;
use crate::errors::NodecosmosError;
use crate::models::flow_step::FlowStep;
use scylla::CachingSession;

pub struct FlowStepIdxCalculator {
    prev_flow_step: Option<FlowStep>,
    next_flow_step: Option<FlowStep>,
}

impl FlowStepIdxCalculator {
    pub async fn new(db_session: &CachingSession, params: &FlowStepCreationParams) -> Result<Self, NodecosmosError> {
        let mut prev_flow_step = None;
        let mut next_flow_step = None;

        if let Some(pref_flow_step_id) = &params.pref_flow_step_id {
            let fs = FlowStep::find_by_node_id_and_id(db_session, params.node_id, *pref_flow_step_id).await?;
            prev_flow_step = Some(fs);
        }

        if let Some(next_flow_step_id) = &params.next_flow_step_id {
            let fs = FlowStep::find_by_node_id_and_id(db_session, params.node_id, *next_flow_step_id).await?;
            next_flow_step = Some(fs);
        }

        Ok(Self {
            prev_flow_step,
            next_flow_step,
        })
    }

    pub fn validate(&self) -> Result<(), NodecosmosError> {
        match (&self.prev_flow_step, &self.next_flow_step) {
            (Some(prev_flow_step), Some(next_flow_step)) => match prev_flow_step.next_flow_step_id {
                Some(prev_flow_step_next_flow_step_id) => {
                    if prev_flow_step_next_flow_step_id != next_flow_step.id {
                        return Err(NodecosmosError::Conflict(format!(
                            r#"
                                The previous flow step's next flow step id ({}) 
                                does not match the next flow step's id ({})
                            "#,
                            prev_flow_step_next_flow_step_id, next_flow_step.id
                        )));
                    }
                }
                None => {
                    return Err(NodecosmosError::Conflict(format!(
                        "The previous flow step's next flow step id is null"
                    )));
                }
            },
            (Some(prev_flow_step), None) => {
                if prev_flow_step.next_flow_step_id.is_some() {
                    return Err(NodecosmosError::Conflict(format!(
                        "The previous flow step's next flow step id is not null"
                    )));
                }
            }
            (None, Some(next_flow_step)) => {
                if next_flow_step.prev_flow_step_id.is_some() {
                    return Err(NodecosmosError::Conflict(format!(
                        "The next flow step's previous flow step id is not null"
                    )));
                }
            }
            (None, None) => {}
        }

        Ok(())
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
