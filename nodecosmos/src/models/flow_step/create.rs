use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::flow_step::FlowStep;
use charybdis::model::AsNative;
use charybdis::operations::{Find, Insert, UpdateWithCallbacks};
use charybdis::types::Uuid;
use scylla::CachingSession;

impl FlowStep {
    pub fn set_defaults(&mut self) {
        let now = chrono::Utc::now();

        self.id = Uuid::new_v4();
        self.created_at = Some(now);
        self.updated_at = Some(now);
    }

    pub async fn create_branched_if_original_exists(&self, data: &RequestData) -> Result<(), NodecosmosError> {
        let mut maybe_original = FlowStep {
            node_id: self.node_id,
            branch_id: self.branch_id,
            flow_id: self.flow_id,
            flow_index: self.flow_index,
            id: self.id,
            ..Default::default()
        }
        .maybe_find_by_primary_key()
        .execute(data.db_session())
        .await?;

        if let Some(maybe_original) = maybe_original.as_mut() {
            maybe_original.branch_id = self.branch_id;

            maybe_original.insert().execute(data.db_session()).await?;

            return Ok(());
        }

        Ok(())
    }

    pub async fn validate_conflicts(&mut self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        let prev_flow_step = self.prev_flow_step(db_session).await?;
        let next_flow_step = self.next_flow_step(db_session).await?;

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
                    return Err(NodecosmosError::Conflict(
                        "The previous flow step's next flow step id is null".to_string(),
                    ));
                }
            },
            (Some(prev_flow_step), None) => {
                if prev_flow_step.next_flow_step_id.is_some() {
                    return Err(NodecosmosError::InternalServerError(format!(
                        "The previous flow step's next flow step id is not null. \
                         BranchId: {:?} Prev id: {:?}  next id: {:?} ",
                        self.branch_id, prev_flow_step.id, prev_flow_step.next_flow_step_id
                    )));
                }
            }
            (None, Some(next_flow_step)) => {
                if next_flow_step.prev_flow_step_id.is_some() {
                    return Err(NodecosmosError::Conflict(
                        "The next flow step's previous flow step id is not null".to_string(),
                    ));
                }
            }
            (None, None) => {}
        }

        Ok(())
    }

    pub async fn calculate_index(&mut self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        let prev_flow_step = self.prev_flow_step(db_session).await?;
        let next_flow_step = self.next_flow_step(db_session).await?;

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
    pub async fn sync_surrounding_fs_on_creation(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        let mut prev_flow_step = self.prev_flow_step(data.db_session()).await?;
        let mut next_flow_step = self.next_flow_step(data.db_session()).await?;

        match (prev_flow_step.as_mut(), next_flow_step.as_mut()) {
            (Some(prev_fs), Some(next_fs)) => {
                let mut prev_fs = prev_fs.as_native();
                let mut next_fs = next_fs.as_native();

                prev_fs.pull_outputs_from_next_flow_step(data).await?;
                prev_fs.next_flow_step_id = Some(self.id);
                prev_fs.update_cb(data).execute(data.db_session()).await?;

                next_fs.prev_flow_step_id = Some(self.id);
                next_fs.update_cb(data).execute(data.db_session()).await?;
                next_fs.remove_inputs(data).await?;
            }
            (Some(prev_fs), None) => {
                let mut prev_fs = prev_fs.as_native();

                prev_fs.next_flow_step_id = Some(self.id);
                prev_fs.update_cb(data).execute(data.db_session()).await?;
            }
            (None, Some(next_fs)) => {
                let mut next_fs = next_fs.as_native();

                next_fs.prev_flow_step_id = Some(self.id);
                next_fs.update_cb(data).execute(data.db_session()).await?;
                next_fs.remove_inputs(data).await?;
            }
            _ => {}
        }

        Ok(())
    }
}
