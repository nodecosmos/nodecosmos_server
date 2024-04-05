use crate::api::data::RequestData;
use crate::api::WorkflowParams;
use crate::errors::NodecosmosError;
use crate::models::branch::update::BranchUpdate;
use crate::models::branch::Branch;
use crate::models::flow::Flow;
use crate::models::flow_step::FlowStep;
use crate::models::traits::ModelContext;
use crate::models::traits::{Branchable, FindOrInsertBranchedFromParams};
use charybdis::batch::ModelBatch;
use charybdis::model::AsNative;
use charybdis::operations::{Find, Insert, UpdateWithCallbacks};
use charybdis::types::Uuid;

impl FlowStep {
    pub fn set_defaults(&mut self) {
        if self.is_default_context() {
            let now = chrono::Utc::now();

            self.id = Uuid::new_v4();
            self.created_at = now;
            self.updated_at = now;
        }
    }

    pub async fn create_branched_if_original_exists(&self, data: &RequestData) -> Result<(), NodecosmosError> {
        if self.is_branched() {
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
            }
        }

        Ok(())
    }

    pub async fn validate_conflicts(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        if self.is_parent_delete_context() && !self.siblings_already_in_sync(data).await? {
            return Ok(());
        }

        println!("Validating conflicts");

        let prev_flow_step = self.prev_flow_step(data).await?.lock()?;
        println!("prev_flow_step");
        let prev_flow_step_id = prev_flow_step.as_ref().map(|fs| fs.id);
        let prev_flow_step_next_flow_step_id = prev_flow_step.as_ref().map(|fs| fs.next_flow_step_id);

        println!("find next flow step {:?}", self.next_flow_step_id);
        let next_flow_step = self.next_flow_step(data).await?.lock()?;
        println!("next_flow_step");
        let next_flow_step_id = next_flow_step.as_ref().map(|fs| fs.id);
        let next_flow_step_prev_flow_step_id = next_flow_step.as_ref().map(|fs| fs.prev_flow_step_id);

        match (prev_flow_step_id, next_flow_step_id) {
            (Some(_), Some(_)) => {
                if prev_flow_step_next_flow_step_id != Some(next_flow_step_id) {
                    return Err(NodecosmosError::Conflict(format!(
                        r#"
                        The previous flow step's next flow step id ({:?})
                        does not match the next flow step's prev_flow_step id ({:?})
                        "#,
                        prev_flow_step_next_flow_step_id, next_flow_step_prev_flow_step_id
                    )));
                }

                if next_flow_step_prev_flow_step_id != Some(prev_flow_step_id) {
                    return Err(NodecosmosError::Conflict(format!(
                        r#"
                        The next flow step's prev flow step id ({:?})
                        does not match the previous flow step's next_flow_step id ({:?})
                        "#,
                        next_flow_step_prev_flow_step_id, prev_flow_step_next_flow_step_id
                    )));
                }
            }
            (Some(prev_flow_step_id), None) => {
                if prev_flow_step_next_flow_step_id.is_some() {
                    return Err(NodecosmosError::Conflict(format!(
                        "The previous flow step's next flow step id is not null.
                         BranchId: {:?} Prev id: {:?}  next id: {:?} ",
                        self.branch_id, prev_flow_step_id, prev_flow_step_next_flow_step_id
                    )));
                }
            }
            (None, Some(_)) => {
                if next_flow_step_prev_flow_step_id.is_some() {
                    return Err(NodecosmosError::Conflict(format!(
                        r#"
                        The next flow step's prev flow step id is not null.
                        BranchId: {:?} Next id: {:?}  prev_id id: {:?},
                        "#,
                        self.branch_id, next_flow_step_id, next_flow_step_prev_flow_step_id
                    )));
                }
            }
            (None, None) => {}
        }

        Ok(())
    }

    pub async fn calculate_index(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        let prev_flow_step_index = self
            .prev_flow_step(data)
            .await?
            .lock()?
            .as_ref()
            .map(|fs| fs.flow_index);
        let next_flow_step_index = self
            .next_flow_step(data)
            .await?
            .lock()?
            .as_ref()
            .map(|fs| fs.flow_index);

        match (prev_flow_step_index, next_flow_step_index) {
            (Some(prev_flow_step_index), Some(next_flow_step_index)) => {
                self.flow_index = (prev_flow_step_index + next_flow_step_index) / 2.0;
            }
            (Some(prev_flow_step_index), None) => {
                self.flow_index = prev_flow_step_index + 1.0;
            }
            (None, Some(next_flow_step_index)) => {
                self.flow_index = next_flow_step_index - 1.0;
            }
            (None, None) => {
                self.flow_index = 0.0;
            }
        }

        return Ok(());
    }

    pub async fn sync_surrounding_fs_on_creation(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        if self.siblings_already_in_sync(data).await? {
            return Ok(());
        }

        let mut prev_flow_step = self
            .prev_flow_step(data)
            .await?
            .lock()?
            .as_ref()
            .map(|fs| fs.as_native());
        let mut next_flow_step = self
            .next_flow_step(data)
            .await?
            .lock()?
            .as_ref()
            .map(|fs| fs.as_native());

        match (prev_flow_step.as_mut(), next_flow_step.as_mut()) {
            (Some(prev_fs), Some(next_fs)) => {
                let _ = prev_fs.pull_outputs_from_next_flow_step(data).await.map_err(|e| {
                    log::warn!("Failed to pull outputs from next flow step: {:?}", e);
                    e
                })?;

                let _ = next_fs.remove_inputs(data).await.map_err(|e| {
                    log::warn!("Failed to remove inputs from next flow step: {:?}", e);
                    e
                });

                prev_fs.next_flow_step_id = Some(self.id);
                prev_fs.updated_at = chrono::Utc::now();
                next_fs.prev_flow_step_id = Some(self.id);
                next_fs.updated_at = chrono::Utc::now();

                FlowStep::batch()
                    .append_update(prev_fs)
                    .append_update(next_fs)
                    .execute(data.db_session())
                    .await?;
            }
            (Some(prev_fs), None) => {
                prev_fs.next_flow_step_id = Some(self.id);
                prev_fs.update_cb(data).execute(data.db_session()).await?;
            }
            (None, Some(next_fs)) => {
                next_fs.prev_flow_step_id = Some(self.id);
                next_fs.update_cb(data).execute(data.db_session()).await?;
                let _ = next_fs.remove_inputs(data).await.map_err(|e| {
                    log::warn!("[next_fs] Failed to remove inputs from next flow step: {:?}", e);
                    e
                });
            }
            _ => {}
        }

        Ok(())
    }

    pub async fn preserve_branch_flow(&self, data: &RequestData) -> Result<(), NodecosmosError> {
        if self.is_branched() {
            Flow::find_or_insert_branched(
                data,
                &WorkflowParams {
                    node_id: self.node_id,
                    branch_id: self.branch_id,
                },
                self.flow_id,
            )
            .await?;
        }

        Ok(())
    }

    pub async fn update_branch_with_creation(&self, data: &RequestData) -> Result<(), NodecosmosError> {
        if self.is_branched() {
            Branch::update(data, self.branch_id, BranchUpdate::EditNodeWorkflow(self.node_id)).await?;
            Branch::update(data, self.branch_id, BranchUpdate::CreateFlowStep(self.id)).await?;
        }

        Ok(())
    }

    pub async fn update_branch_with_deletion(&self, data: &RequestData) -> Result<(), NodecosmosError> {
        if self.is_branched() {
            Branch::update(data, self.branch_id, BranchUpdate::EditNodeWorkflow(self.node_id)).await?;
            Branch::update(data, self.branch_id, BranchUpdate::DeleteFlowStep(self.id)).await?;
        }

        Ok(())
    }
}
