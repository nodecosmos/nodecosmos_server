use crate::errors::NodecosmosError;
use crate::models::flow_step::FlowStep;
use charybdis::operations::UpdateWithCallbacks;
use scylla::CachingSession;

impl FlowStep {
    pub async fn validate_conflicts(&mut self, session: &CachingSession) -> Result<(), NodecosmosError> {
        let prev_flow_step = self.prev_flow_step(session).await?;
        let next_flow_step = self.next_flow_step(session).await?;

        match (prev_flow_step.as_ref(), next_flow_step.as_ref()) {
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

    pub async fn calculate_index(&mut self, session: &CachingSession) -> Result<(), NodecosmosError> {
        let prev_flow_step = self.prev_flow_step(session).await?;
        let next_flow_step = self.next_flow_step(session).await?;

        if prev_flow_step.is_none() && next_flow_step.is_none() {
            self.flow_index = 0.0;
        }

        let prev_flow_step_index = if let Some(prev_flow_step) = prev_flow_step.as_ref() {
            prev_flow_step.flow_index
        } else {
            0.0
        };

        let next_flow_step_index = if let Some(next_flow_step) = next_flow_step.as_ref() {
            next_flow_step.flow_index
        } else {
            0.0
        };

        // If only the next flow step exists, return its flow index minus 1
        if prev_flow_step.is_none() {
            self.flow_index = next_flow_step_index - 1.0;

            return Ok(());
        }

        // If only the prev flow step exists, return its flow index plus 1
        if next_flow_step.is_none() {
            self.flow_index = prev_flow_step_index + 1.0;

            return Ok(());
        }

        // If both prev and next flow steps exist, return the average of their flow indices
        self.flow_index = (prev_flow_step_index + next_flow_step_index) / 2.0;

        return Ok(());
    }

    // syncs the prev and next flow steps when a new flow step is created
    pub async fn sync_surrounding_fs_on_creation(&mut self, session: &CachingSession) -> Result<(), NodecosmosError> {
        let mut prev_flow_step = self.prev_flow_step(session).await?.as_ref().clone();
        let mut next_flow_step = self.next_flow_step(session).await?.as_ref().clone();

        match (prev_flow_step.as_mut(), next_flow_step.as_mut()) {
            (Some(prev_fs), Some(next_fs)) => {
                prev_fs.pull_outputs_from_next_flow_step(session).await?;
                prev_fs.next_flow_step_id = Some(self.id);
                prev_fs.update_cb(session).await?;

                next_fs.prev_flow_step_id = Some(self.id);
                next_fs.update_cb(session).await?;
                next_fs.remove_inputs(session).await?;
            }
            (Some(prev_fs), None) => {
                prev_fs.next_flow_step_id = Some(self.id);
                prev_fs.update_cb(session).await?;
            }
            (None, Some(next_fs)) => {
                next_fs.prev_flow_step_id = Some(self.id);
                next_fs.update_cb(session).await?;
                next_fs.remove_inputs(session).await?;
            }
            _ => {}
        }

        Ok(())
    }
}
