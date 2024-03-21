use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::branch::update::BranchUpdate;
use crate::models::branch::Branch;
use crate::models::flow::DeleteFlow;
use crate::models::flow_step::DeleteFlowStep;
use crate::models::input_output::DeleteIo;
use crate::models::node::Node;
use crate::models::traits::node::FindBranched;
use crate::models::traits::Branchable;
use crate::models::utils::updated_at_cb_fn;
use crate::models::workflow::{UpdateInitialInputsWorkflow, UpdateWorkflowTitle, Workflow};
use charybdis::callbacks::Callbacks;
use charybdis::model::AsNative;
use scylla::CachingSession;

impl Callbacks for Workflow {
    type Extension = RequestData;
    type Error = NodecosmosError;

    async fn before_insert(&mut self, db_session: &CachingSession, data: &RequestData) -> Result<(), NodecosmosError> {
        let now = chrono::Utc::now();

        self.id = charybdis::types::Uuid::new_v4();
        self.created_at = Some(now);
        self.updated_at = Some(now);

        if self.is_branched() {
            let node = Node::find_branched_or_original(db_session, self.node_id, self.branch_id, None).await?;
            node.create_branched_if_not_exist(data).await?;

            Branch::update(data, self.branch_id, BranchUpdate::CreateWorkflow(self.id)).await?;
        }

        Ok(())
    }

    updated_at_cb_fn!();

    async fn before_delete(&mut self, _db_session: &CachingSession, data: &RequestData) -> Result<(), Self::Error> {
        if self.is_branched() {
            Branch::update(data, self.branch_id, BranchUpdate::DeleteWorkflow(self.id)).await?;
        }

        Ok(())
    }

    async fn after_delete(&mut self, db_session: &CachingSession, data: &RequestData) -> Result<(), NodecosmosError> {
        if self.is_branched() {
            self.create_branched_if_original_exists(data).await?;
        }

        DeleteFlowStep::delete_by_node_id_and_branch_id_and_workflow_id(self.node_id, self.branch_id, self.id)
            .execute(db_session)
            .await?;

        DeleteFlow::delete_by_node_id_and_branch_id_and_workflow_id(self.node_id, self.branch_id, self.id)
            .execute(db_session)
            .await?;

        // NOTE: if we allow multiple workflows per node, we need to delete only the io that belongs to this workflow
        DeleteIo::delete_by_root_node_id_and_branch_id_and_node_id(
            self.root_node_id,
            self.branchise_id(self.root_node_id),
            self.node_id,
        )
        .execute(db_session)
        .await?;

        Ok(())
    }
}

impl Callbacks for UpdateInitialInputsWorkflow {
    type Extension = RequestData;
    type Error = NodecosmosError;

    async fn before_update(&mut self, _db_session: &CachingSession, data: &RequestData) -> Result<(), NodecosmosError> {
        self.updated_at = Some(chrono::Utc::now());

        if self.is_branched() {
            self.as_native().create_branched_if_not_exist(data).await?;
            self.update_branch(data).await?;
        }

        Ok(())
    }
}

impl Callbacks for UpdateWorkflowTitle {
    type Extension = RequestData;
    type Error = NodecosmosError;

    async fn before_update(&mut self, _db_session: &CachingSession, data: &RequestData) -> Result<(), NodecosmosError> {
        self.updated_at = Some(chrono::Utc::now());

        if self.is_branched() {
            self.as_native().create_branched_if_not_exist(data).await?;
            Branch::update(data, self.branch_id, BranchUpdate::EditWorkflowTitle(self.id)).await?;
        }

        Ok(())
    }
}
