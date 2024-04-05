mod update_initial_inputs;

use crate::api::data::RequestData;
use crate::api::WorkflowParams;
use crate::errors::NodecosmosError;
use crate::models::input_output::Io;
use crate::models::node::Node;
use crate::models::traits::node::FindBranched;
use crate::models::traits::Branchable;
use crate::models::traits::Context;
use charybdis::callbacks::Callbacks;
use charybdis::macros::charybdis_model;
use charybdis::types::{List, Text, Timestamp, Uuid};
use nodecosmos_macros::Branchable;
use scylla::CachingSession;
use serde::{Deserialize, Serialize};

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
#[charybdis_model(
    table_name = workflows,
    partition_keys = [node_id],
    clustering_keys = [branch_id],
    global_secondary_indexes = []
)]
#[derive(Branchable, Serialize, Deserialize, Default, Clone)]
pub struct Workflow {
    #[serde(rename = "nodeId")]
    #[branch(original_id)]
    pub node_id: Uuid,

    #[serde(rename = "branchId")]
    pub branch_id: Uuid,

    #[serde(rename = "rootId")]
    pub root_id: Uuid,

    pub title: Option<Text>,

    #[serde(rename = "initialInputIds")]
    pub initial_input_ids: Option<List<Uuid>>,

    #[serde(rename = "createdAt", default = "chrono::Utc::now")]
    pub created_at: Timestamp,

    #[serde(rename = "updatedAt", default = "chrono::Utc::now")]
    pub updated_at: Timestamp,

    #[charybdis(ignore)]
    #[serde(skip)]
    pub ctx: Context,
}

impl Callbacks for Workflow {
    type Extension = RequestData;
    type Error = NodecosmosError;

    async fn before_insert(&mut self, db_session: &CachingSession, data: &RequestData) -> Result<(), NodecosmosError> {
        if self.is_branched() {
            let node = Node::find_branched_or_original(db_session, self.node_id, self.branch_id, None).await?;
            node.create_branched_if_not_exist(data).await?;
        }

        Ok(())
    }
}

impl Workflow {
    pub async fn branched(db_session: &CachingSession, params: &WorkflowParams) -> Result<Workflow, NodecosmosError> {
        let original = Workflow::find_first_by_node_id_and_branch_id(params.node_id, params.node_id)
            .execute(db_session)
            .await?;

        if params.is_original() {
            Ok(original)
        } else {
            let maybe_branched = Workflow::maybe_find_first_by_node_id_and_branch_id(params.node_id, params.branch_id)
                .execute(db_session)
                .await?;

            let mut branched;

            if let Some(mut maybe_branched) = maybe_branched {
                // merge original initial input ids with branched initial input ids
                if let Some(original_initial_input_ids) = original.initial_input_ids {
                    let mut branched_initial_input_ids = maybe_branched.initial_input_ids.unwrap_or_default();
                    branched_initial_input_ids.extend(original_initial_input_ids);
                    maybe_branched.initial_input_ids = Some(branched_initial_input_ids);
                }

                branched = maybe_branched;
            } else {
                branched = original;
                branched.branch_id = params.branch_id;
            }

            Ok(branched)
        }
    }

    pub async fn input_outputs(&self, db_session: &CachingSession) -> Result<Vec<Io>, NodecosmosError> {
        Io::branched(
            db_session,
            self.root_id,
            &WorkflowParams {
                node_id: self.node_id,
                branch_id: self.branch_id,
            },
        )
        .await
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
