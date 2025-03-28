use charybdis::batch::ModelBatch;
use charybdis::operations::{Find, Insert};
use scylla::client::caching_session::CachingSession;

use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::branch::update::BranchUpdate;
use crate::models::branch::Branch;
use crate::models::flow_step::{FlowStep, UpdateOutputIdsFlowStep};
use crate::models::io::Io;
use crate::models::node::Node;
use crate::models::traits::{Branchable, FindOrInsertBranched, ModelBranchParams, ModelContext, NodeBranchParams};
use crate::models::workflow::UpdateInitialInputsWorkflow;

impl Io {
    pub async fn push_to_initial_input_ids(&self, data: &RequestData) -> Result<(), NodecosmosError> {
        if self.initial_input {
            UpdateInitialInputsWorkflow {
                branch_id: self.branch_id,
                node_id: self.node_id,
                root_id: self.root_id,
                ..Default::default()
            }
            .push_initial_input(data.db_session(), self.id)
            .await?;
        }

        Ok(())
    }

    pub async fn push_to_flow_step_outputs(&self, data: &RequestData) -> Result<(), NodecosmosError> {
        if let (Some(flow_step_id), Some(flow_step_node_id)) = (self.flow_step_id, self.flow_step_node_id) {
            UpdateOutputIdsFlowStep::find_first_by_branch_id_and_id(self.branch_id, flow_step_id)
                .execute(data.db_session())
                .await?
                .push_output(data, flow_step_node_id, self.id)
                .await?;
        }

        Ok(())
    }

    pub async fn create_branched_if_original_exists(&self, data: &RequestData) -> Result<(), NodecosmosError> {
        if self.is_branch() {
            let mut maybe_original = Io {
                root_id: self.root_id,
                branch_id: self.original_id(),
                node_id: self.node_id,
                id: self.id,
                ..Default::default()
            }
            .maybe_find_by_primary_key()
            .execute(data.db_session())
            .await?;

            if let Some(maybe_branched) = maybe_original.as_mut() {
                maybe_branched.branch_id = self.branch_id;

                if maybe_branched
                    .maybe_find_by_primary_key()
                    .execute(data.db_session())
                    .await?
                    .is_none()
                {
                    maybe_branched.insert().execute(data.db_session()).await?;
                }
            }
        }

        Ok(())
    }

    pub async fn validate_attributes(&mut self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        let node = self.node(db_session).await?;
        let root_id = node.root_id;

        if self.root_id != root_id {
            return Err(NodecosmosError::Unauthorized("Not authorized to add IO for this node!"));
        }

        if self.title.is_none() {
            return Err(NodecosmosError::BadRequest("Title is required!".to_string()));
        }

        Ok(())
    }

    pub async fn copy_vals_from_main(&mut self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        let main_io = self.main_io(db_session).await?;

        if let Some(main_io) = main_io {
            self.title = main_io.title;
            self.unit = main_io.unit;
            self.data_type = main_io.data_type;
            self.main_id = main_io.main_id;
        } else {
            self.main_id = Some(self.id);
        }

        Ok(())
    }

    pub async fn clone_main_ios_to_branch(&self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        let branched: Vec<Io> = Io::branched(
            db_session,
            &NodeBranchParams {
                root_id: self.root_id,
                branch_id: self.branch_id,
                node_id: self.node_id,
            },
        )
        .await?
        .into_iter()
        .filter(|io| io.main_id == self.main_id)
        .collect();

        Io::unlogged_batch()
            .chunked_insert(db_session, &branched, crate::constants::BATCH_CHUNK_SIZE)
            .await?;

        Ok(())
    }

    pub async fn preserve_branch_node(&self, data: &RequestData) -> Result<(), NodecosmosError> {
        if self.is_branch() {
            Node::find_or_insert_branched(
                data,
                ModelBranchParams {
                    original_id: self.original_id(),
                    branch_id: self.branch_id,
                    id: self.node_id,
                },
            )
            .await?;
        }

        Ok(())
    }

    pub async fn preserve_branch_flow_step(&self, data: &RequestData) -> Result<(), NodecosmosError> {
        if self.is_branch() && !self.is_parent_delete_context() {
            if let Some(flow_step_id) = self.flow_step_id {
                FlowStep::find_or_insert_branched(
                    data,
                    ModelBranchParams {
                        original_id: self.original_id(),
                        branch_id: self.branch_id,
                        id: flow_step_id,
                    },
                )
                .await?;
            }
        }

        Ok(())
    }

    pub async fn update_branch_with_creation(&self, data: &RequestData) -> Result<(), NodecosmosError> {
        if self.is_branch() {
            Branch::update(data.db_session(), self.branch_id, BranchUpdate::EditNode(self.node_id)).await?;
            Branch::update(data.db_session(), self.branch_id, BranchUpdate::CreateIo(self.id)).await?;
        }

        Ok(())
    }

    pub async fn update_branch_with_deletion(&self, data: &RequestData) -> Result<(), NodecosmosError> {
        if self.is_branch() && !self.is_parent_delete_context() {
            Branch::update(data.db_session(), self.branch_id, BranchUpdate::EditNode(self.node_id)).await?;
            Branch::update(data.db_session(), self.branch_id, BranchUpdate::DeleteIo(self.id)).await?;
        }

        Ok(())
    }
}
