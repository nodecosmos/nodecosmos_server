use charybdis::callbacks::Callbacks;
use charybdis::macros::charybdis_model;
use charybdis::stream::CharybdisModelStream;
use charybdis::types::{List, Text, Timestamp, Uuid};
use scylla::CachingSession;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use nodecosmos_macros::Branchable;

use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::traits::{Branchable, Merge, NodeBranchParams};

mod update_initial_inputs;

/// ### Workflow structure
/// - Each `Workflow` has multiple `Flows`
/// - Each `Flow` represents isolated process within the `Workflow`
/// - Each `Flow` has multiple `FlowSteps`
/// - Each `FlowStep` represents a single step within a `Flow`
///
/// In back-end we don't have `WorkflowSteps` so in front-end we have to build
/// them by understanding that each `WorkflowStep` have corresponding `FlowSteps` that are calculated
/// by `Flow.startIndex` + index of a `FlowStep` within `Flow`.
/// Each Flow starting position, within the `Workflow`, is determined by `flow.startIndex` attribute.
///
/// We support single workflow per node. In future we could get rid of this table and move initial_inputs
/// to the node table. The reason we have it here is because initially we wanted to support multiple workflows per node.
#[charybdis_model(
    table_name = workflows,
    partition_keys = [branch_id],
    clustering_keys = [node_id],
    global_secondary_indexes = []
)]
#[derive(Branchable, Serialize, Deserialize, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Workflow {
    #[branch(original_id)]
    pub node_id: Uuid,

    pub branch_id: Uuid,
    pub root_id: Uuid,
    pub title: Option<Text>,
    pub initial_input_ids: Option<List<Uuid>>,

    #[serde(default = "chrono::Utc::now")]
    pub created_at: Timestamp,

    #[serde(default = "chrono::Utc::now")]
    pub updated_at: Timestamp,
}

impl Workflow {
    pub async fn branched(db_session: &CachingSession, params: &NodeBranchParams) -> Result<Workflow, NodecosmosError> {
        let mut original = Workflow::find_by_branch_id_and_node_id(params.original_id, params.node_id)
            .execute(db_session)
            .await?;

        if params.is_original() {
            Ok(original)
        } else {
            let maybe_branched = Workflow::maybe_find_first_by_branch_id_and_node_id(params.branch_id, params.node_id)
                .execute(db_session)
                .await?;

            let mut branched;

            if let Some(mut maybe_branched) = maybe_branched {
                // merge original initial input ids with branched initial input ids
                if let Some(original_initial_input_ids) = original.initial_input_ids.as_mut() {
                    original_initial_input_ids.merge_unique(maybe_branched.initial_input_ids.unwrap_or_default());
                    maybe_branched.initial_input_ids = original.initial_input_ids;
                }

                branched = maybe_branched;
            } else {
                branched = original;
                branched.branch_id = params.branch_id;
            }

            Ok(branched)
        }
    }

    pub async fn find_by_node_ids(
        db_session: &CachingSession,
        branch_id: Uuid,
        node_ids: &HashSet<Uuid>,
    ) -> Result<CharybdisModelStream<Workflow>, NodecosmosError> {
        find_workflow!("branch_id = ? AND node_id IN ?", (branch_id, node_ids))
            .execute(db_session)
            .await
            .map_err(NodecosmosError::from)
    }
}

partial_workflow!(
    UpdateInitialInputsWorkflow,
    node_id,
    branch_id,
    initial_input_ids,
    updated_at
);

partial_workflow!(GetInitialInputsWorkflow, node_id, branch_id, initial_input_ids);

impl Callbacks for UpdateInitialInputsWorkflow {
    type Extension = RequestData;
    type Error = NodecosmosError;

    async fn before_update(&mut self, _db_session: &CachingSession, data: &RequestData) -> Result<(), NodecosmosError> {
        if self.is_branched() {
            self.update_branch(data).await?;
        }

        Ok(())
    }
}

// used by node deletion
partial_workflow!(DeleteWorkflow, node_id, branch_id);

partial_workflow!(UpdateWorkflowTitle, node_id, branch_id, title, updated_at);
