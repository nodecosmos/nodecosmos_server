use charybdis::operations::{Find, Insert};
use charybdis::types::Uuid;

use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::branch::update::BranchUpdate;
use crate::models::branch::Branch;
use crate::models::flow::Flow;
use crate::models::flow_step::{FlowStep, PkFlowStep};
use crate::models::traits::{Branchable, FindOrInsertBranched};
use crate::models::traits::{ModelBranchParams, ModelContext};

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
        if self.is_branch() {
            let mut maybe_original = FlowStep {
                node_id: self.node_id,
                branch_id: self.original_id(),
                flow_id: self.flow_id,
                step_index: self.step_index.clone(),
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
        if PkFlowStep::from(&*self)
            .maybe_find_by_index(data.db_session())
            .await?
            .is_some()
        {
            return Err(NodecosmosError::Conflict(format!(
                "Flow Step on given index {} already exists",
                self.step_index
            )));
        }

        Ok(())
    }

    pub async fn preserve_branch_flow(&self, data: &RequestData) -> Result<(), NodecosmosError> {
        if self.is_branch() {
            Flow::find_or_insert_branched(
                data,
                ModelBranchParams {
                    original_id: self.original_id(),
                    branch_id: self.branch_id,
                    id: self.flow_id,
                },
            )
            .await?;
        }

        Ok(())
    }

    pub async fn update_branch_with_creation(&self, data: &RequestData) -> Result<(), NodecosmosError> {
        if self.is_branch() {
            Branch::update(data.db_session(), self.branch_id, BranchUpdate::EditNode(self.node_id)).await?;
            Branch::update(data.db_session(), self.branch_id, BranchUpdate::CreateFlowStep(self.id)).await?;
        }

        Ok(())
    }

    pub async fn update_branch_with_deletion(&self, data: &RequestData) -> Result<(), NodecosmosError> {
        if self.is_branch() {
            Branch::update(data.db_session(), self.branch_id, BranchUpdate::EditNode(self.node_id)).await?;
            Branch::update(data.db_session(), self.branch_id, BranchUpdate::DeleteFlowStep(self.id)).await?;
        }

        Ok(())
    }
}
