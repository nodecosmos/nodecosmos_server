use charybdis::operations::{Find, Insert};
use charybdis::types::Uuid;

use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::branch::update::BranchUpdate;
use crate::models::branch::Branch;
use crate::models::flow::Flow;
use crate::models::flow_step::{FlowStep, PkFlowStep};
use crate::models::io::Io;
use crate::models::node::Node;
use crate::models::traits::{Branchable, FindOrInsertBranched, Merge};
use crate::models::traits::{ModelBranchParams, ModelContext};
use crate::models::utils::process_in_chunks;

impl FlowStep {
    pub fn set_defaults(&mut self) {
        if self.is_default_context() {
            let now = chrono::Utc::now();

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

    pub async fn save_original_data_to_branch(
        &self,
        data: &RequestData,
        branch_id: Uuid,
    ) -> Result<(), NodecosmosError> {
        if self.is_original() {
            let mut clone = self.clone();
            clone.branch_id = branch_id;

            if clone
                .maybe_find_by_primary_key()
                .execute(data.db_session())
                .await?
                .is_none()
            {
                clone.insert().execute(data.db_session()).await?;
            }

            clone.preserve_branch_flow(data).await?;
            clone.preserve_flow_step_ios(data).await?;
            clone.preserve_flow_step_nodes(data).await?;
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

    pub async fn preserve_flow_step_ios(&self, data: &RequestData) -> Result<(), NodecosmosError> {
        if self.is_branch() {
            let mut io_ids_by_node_id = self.output_ids_by_node_id.clone();
            io_ids_by_node_id.merge(self.input_ids_by_node_id.clone());

            if let Some(io_ids_by_node_id) = io_ids_by_node_id {
                let output_ids = io_ids_by_node_id.values().flatten().cloned().collect::<Vec<Uuid>>();

                process_in_chunks(output_ids, |output_id| async move {
                    let output = Io {
                        root_id: self.root_id,
                        branch_id: self.branch_id,
                        node_id: self.node_id,
                        id: output_id,
                        flow_step_id: Some(self.id),
                        ..Default::default()
                    };

                    output.create_branched_if_original_exists(data).await
                })
                .await?;
            }
        }

        Ok(())
    }

    pub async fn preserve_flow_step_nodes(&self, data: &RequestData) -> Result<(), NodecosmosError> {
        if self.is_branch() {
            let node_ids = self
                .input_ids_by_node_id
                .clone()
                .unwrap_or_default()
                .into_keys()
                .chain(self.output_ids_by_node_id.clone().unwrap_or_default().into_keys())
                .collect::<Vec<Uuid>>();

            process_in_chunks(node_ids, |node_id| async move {
                Node::find_or_insert_branched(
                    data,
                    ModelBranchParams {
                        original_id: self.original_id(),
                        branch_id: self.branch_id,
                        id: node_id,
                    },
                )
                .await
            })
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
