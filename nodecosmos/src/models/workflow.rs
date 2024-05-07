use charybdis::callbacks::Callbacks;
use charybdis::macros::charybdis_model;
use charybdis::operations::DeleteWithCallbacks;
use charybdis::stream::CharybdisModelStream;
use charybdis::types::{List, Text, Timestamp, Uuid};
use scylla::CachingSession;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use nodecosmos_macros::Branchable;

use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::io::Io;
use crate::models::traits::{Branchable, Context, Merge, ModelContext, NodeBranchParams};

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
    pub node_id: Uuid,

    pub branch_id: Uuid,

    #[branch(original_id)]
    pub root_id: Uuid,

    pub title: Option<Text>,
    pub initial_input_ids: Option<List<Uuid>>,

    #[serde(default = "chrono::Utc::now")]
    pub created_at: Timestamp,

    #[serde(default = "chrono::Utc::now")]
    pub updated_at: Timestamp,

    #[charybdis(ignore)]
    #[serde(skip)]
    pub ctx: Context,
}

impl Workflow {
    pub async fn branched(db_session: &CachingSession, params: &NodeBranchParams) -> Result<Workflow, NodecosmosError> {
        if params.is_original() {
            Workflow::find_by_branch_id_and_node_id(params.original_id(), params.node_id)
                .execute(db_session)
                .await
                .map_err(NodecosmosError::from)
        } else {
            let maybe_original =
                Workflow::maybe_find_first_by_branch_id_and_node_id(params.original_id(), params.node_id)
                    .execute(db_session)
                    .await?;
            let maybe_branched = Workflow::maybe_find_first_by_branch_id_and_node_id(params.branch_id, params.node_id)
                .execute(db_session)
                .await?;

            return match (maybe_original, maybe_branched) {
                (Some(mut original), Some(mut branched)) => {
                    // merge original initial input ids with branched initial input ids
                    if let Some(original_initial_input_ids) = original.initial_input_ids.as_mut() {
                        original_initial_input_ids.merge_unique(branched.initial_input_ids.unwrap_or_default());
                        branched.initial_input_ids = original.initial_input_ids;
                    }

                    Ok(branched)
                }
                (Some(mut original), None) => {
                    original.branch_id = params.branch_id;

                    Ok(original)
                }
                (None, Some(branched)) => Ok(branched),
                (None, None) => Err(NodecosmosError::NotFound(
                    "Branch related workflow not found".to_string(),
                )),
            };
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

partial_workflow!(GetInitialInputsWorkflow, node_id, branch_id, root_id, initial_input_ids);

partial_workflow!(
    UpdateInitialInputsWorkflow,
    node_id,
    branch_id,
    root_id,
    initial_input_ids,
    updated_at,
    ctx
);

impl UpdateInitialInputsWorkflow {
    pub async fn delete_removed_inputs_ios(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        let current_io_ids = if self.is_original() {
            GetInitialInputsWorkflow::find_by_branch_id_and_node_id(self.branch_id, self.node_id)
                .execute(data.db_session())
                .await?
                .initial_input_ids
        } else {
            None
        };

        if let Some(current_io_ids) = current_io_ids {
            for io_id in current_io_ids {
                if self.initial_input_ids.is_none()
                    || self.initial_input_ids.as_ref().is_some_and(|ids| !ids.contains(&io_id))
                {
                    let mut output = Io {
                        root_id: self.root_id,
                        branch_id: self.branch_id,
                        node_id: self.node_id,
                        id: io_id,

                        ..Default::default()
                    };

                    output.set_parent_delete_context();
                    output.delete_cb(data).execute(data.db_session()).await?;
                }
            }
        }

        Ok(())
    }
}

impl Callbacks for UpdateInitialInputsWorkflow {
    type Extension = RequestData;
    type Error = NodecosmosError;

    async fn before_update(&mut self, _db_session: &CachingSession, data: &RequestData) -> Result<(), NodecosmosError> {
        if self.is_branch() {
            self.update_branch(data).await?;
        }

        // In merge context, outputs will be deleted in the next step.
        if !self.is_merge_context() {
            self.delete_removed_inputs_ios(data).await?;
        }

        Ok(())
    }
}

// used by node deletion
partial_workflow!(DeleteWorkflow, node_id, branch_id, root_id);

partial_workflow!(UpdateWorkflowTitle, node_id, branch_id, root_id, title, updated_at);
