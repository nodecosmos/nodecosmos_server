use charybdis::operations::{Find, Insert};

use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::branch::update::BranchUpdate;
use crate::models::branch::Branch;
use crate::models::flow::Flow;
use crate::models::node::Node;
use crate::models::traits::{Branchable, FindOrInsertBranched, ModelBranchParams, NodeBranchParams};

impl Flow {
    pub async fn calculate_vertical_idx(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        let flows = Flow::branched(
            data.db_session(),
            &NodeBranchParams {
                root_id: self.root_id,
                branch_id: self.branch_id,
                node_id: self.node_id,
            },
        )
        .await?;

        self.vertical_index = flows.len() as f64;

        Ok(())
    }

    pub async fn create_branched_if_original_exists(&self, data: &RequestData) -> Result<(), NodecosmosError> {
        if self.is_branched() {
            let mut maybe_original = Flow {
                branch_id: self.original_id(),
                ..self.clone()
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

    pub async fn preserve_branch_node(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        if self.is_branched() {
            Node::find_or_insert_branched(
                data,
                ModelBranchParams {
                    original_id: self.original_id(),
                    branch_id: self.branch_id,
                    node_id: self.node_id,
                    id: self.id,
                },
            )
            .await?;
        }

        Ok(())
    }

    pub async fn update_branch_with_creation(&self, data: &RequestData) -> Result<(), NodecosmosError> {
        if self.is_branched() {
            Branch::update(data, self.branch_id, BranchUpdate::EditNodeWorkflow(self.node_id)).await?;
            Branch::update(data, self.branch_id, BranchUpdate::CreateFlow(self.id)).await?;
        }

        Ok(())
    }

    pub async fn update_branch_with_deletion(&self, data: &RequestData) -> Result<(), NodecosmosError> {
        if self.is_branched() {
            Branch::update(data, self.branch_id, BranchUpdate::EditNodeWorkflow(self.node_id)).await?;
            Branch::update(data, self.branch_id, BranchUpdate::DeleteFlow(self.id)).await?;
        }

        Ok(())
    }
}
