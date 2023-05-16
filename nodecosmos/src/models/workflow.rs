use crate::models::flow::Flow;
use crate::models::helpers::{created_at_cb_fn, updated_at_cb_fn};
use charybdis::*;
use chrono::Utc;
///
/// Workflow model
///
/// Currently we only support one workflow per node,
/// in future we will support multiple workflows per node.
///
/// Single workflow can have multiple flows that can be executed in parallel.
/// Flow should be isolated in a way that represents a single process.
/// Each flow can have multiple flow steps that can be executed in parallel.
/// Each flow step is made of input node and output.
///
/// In that sense Workflow is a collection of flows.
///
/// In future we will allow multiple workflows per node.
/// Reasoning is that we want to allow users to describe multiple processes.
#[partial_model_generator]
#[charybdis_model(
    table_name = "workflows",
    partition_keys = ["node_id"],
    clustering_keys = ["id"],
    secondary_indexes = []
)]
pub struct Workflow {
    #[serde(rename = "nodeId")]
    pub node_id: Uuid,

    pub id: Uuid,
    pub title: Text,
    pub description: Text,

    #[serde(rename = "createdAt")]
    pub created_at: Option<Timestamp>,

    #[serde(rename = "updatedAt")]
    pub updated_at: Option<Timestamp>,

    #[serde(rename = "initialInputIds")]
    pub initial_input_ids: Option<List<Uuid>>,
}

impl Callbacks for Workflow {
    created_at_cb_fn!();

    updated_at_cb_fn!();

    async fn after_delete(&mut self, session: &CachingSession) -> Result<(), CharybdisError> {
        let mut flow = Flow::new();
        flow.node_id = self.node_id;
        flow.workflow_id = self.id;

        let flows = flow.find_by_partition_key(session).await?;

        for mut flow in flows {
            match flow {
                Ok(ref mut flow) => {
                    flow.delete_cb(session).await?;
                }
                Err(e) => {
                    println!(
                        "Workflow::Callbacks::after_delete: Error deleting flow: {:?}",
                        e
                    );
                    continue;
                }
            }
        }

        Ok(())
    }
}
