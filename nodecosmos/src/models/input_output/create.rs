use crate::api::data::RequestData;
use crate::api::WorkflowParams;
use crate::errors::NodecosmosError;
use crate::models::branch::update::BranchUpdate;
use crate::models::branch::Branch;
use crate::models::flow::Flow;
use crate::models::flow_step::FlowStep;
use crate::models::input_output::Io;
use crate::models::traits::{Branchable, FindOrInsertBranchedFromParams};
use charybdis::batch::ModelBatch;
use charybdis::operations::{Find, Insert};
use charybdis::types::Uuid;
use scylla::CachingSession;
use serde_json::json;

impl Io {
    pub async fn create_branched_if_original_exists(&self, data: &RequestData) -> Result<(), NodecosmosError> {
        if self.is_branched() {
            let mut maybe_original = Io {
                root_id: self.root_id,
                branch_id: self.root_id,
                node_id: self.node_id,
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

    pub async fn validate_root_id(&mut self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        let node = self.node(db_session).await?;
        let root_id = node.root_id;

        if self.root_id != root_id {
            return Err(NodecosmosError::Unauthorized(json!({
                "error": "Unauthorized",
                "message": "Not authorized to add IO for this node!"
            })));
        }

        Ok(())
    }

    pub fn set_defaults(&mut self) {
        let now = chrono::Utc::now();

        self.id = Uuid::new_v4();
        self.created_at = Some(now);
        self.updated_at = Some(now);
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
        let branched = Io::branched(
            db_session,
            self.root_id,
            &WorkflowParams {
                node_id: self.node_id,
                branch_id: self.branch_id,
            },
        )
        .await?
        .into_iter()
        .filter(|io| io.main_id == self.main_id)
        .collect();

        Io::unlogged_batch()
            .chunked_insert_if_not_exist(db_session, &branched, 100)
            .await?;

        Ok(())
    }

    pub async fn update_branch_with_creation(&self, data: &RequestData) -> Result<(), NodecosmosError> {
        if self.is_branched() {
            let params = WorkflowParams {
                node_id: self.node_id,
                branch_id: self.branch_id,
            };

            if let Some(flow_step_id) = self.flow_step_id {
                let fs = FlowStep::find_or_insert_branched(data.db_session(), &params, flow_step_id).await?;
                Flow::find_or_insert_branched(data.db_session(), &params, fs.flow_id).await?;
            }

            Branch::update(data, self.branch_id, BranchUpdate::EditNodeWorkflow(self.node_id)).await?;
            Branch::update(data, self.branch_id, BranchUpdate::CreateIo(self.id)).await?;
        }

        Ok(())
    }

    pub async fn update_branch_with_deletion(&self, data: &RequestData) -> Result<(), NodecosmosError> {
        if self.is_branched() {
            Branch::update(data, self.branch_id, BranchUpdate::EditNodeWorkflow(self.node_id)).await?;
            Branch::update(data, self.branch_id, BranchUpdate::DeleteIo(self.id)).await?;
        }

        Ok(())
    }
}
