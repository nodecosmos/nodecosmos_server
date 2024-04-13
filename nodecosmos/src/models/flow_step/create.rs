use crate::api::data::RequestData;
use crate::api::WorkflowParams;
use crate::errors::NodecosmosError;
use crate::models::branch::update::BranchUpdate;
use crate::models::branch::Branch;
use crate::models::flow::Flow;
use crate::models::flow_step::FlowStep;
use crate::models::traits::ModelContext;
use crate::models::traits::{Branchable, FindOrInsertBranchedFromParams};
use charybdis::operations::{Find, Insert};
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
                branch_id: self.original_id(),
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

    pub async fn validate_no_conflicts(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        if self.maybe_find_by_flow_index(data.db_session()).await?.is_some() {
            return Err(NodecosmosError::Conflict(format!(
                "Flow Step on given index {} already exists",
                self.flow_index.to_string()
            )));
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
