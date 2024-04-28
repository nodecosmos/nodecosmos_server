use crate::models::flow_step::FlowStep;
use charybdis::macros::charybdis_model;
use charybdis::types::{Decimal, Frozen, List, Map, Timestamp, Uuid};
use nodecosmos_macros::{Branchable, FlowId, Id, NodeId};
use serde::{Deserialize, Serialize};

/// Node version acts as snapshot of the current state of the node.
/// We keep versions only for node where change has been made, so we
/// don't duplicate data for each ancestor.
#[charybdis_model(
    table_name = archived_flow_steps,
    partition_keys = [branch_id],
    clustering_keys = [node_id, flow_id, step_index, id],
    table_options = r#"
        compression = {
            'sstable_compression': 'SnappyCompressor',
            'chunk_length_in_kb': 64
        }
    "#
)]
#[derive(Branchable, Id, NodeId, FlowId, Serialize, Deserialize, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ArchivedFlowStep {
    #[branch(original_id)]
    pub node_id: Uuid,

    pub branch_id: Uuid,
    pub flow_id: Uuid,

    #[serde(default)]
    pub step_index: Decimal,

    #[serde(default = "Uuid::new_v4")]
    pub id: Uuid,

    pub root_id: Uuid,
    pub node_ids: Option<List<Uuid>>,
    pub input_ids_by_node_id: Option<Frozen<Map<Uuid, Frozen<List<Uuid>>>>>,
    pub output_ids_by_node_id: Option<Frozen<Map<Uuid, Frozen<List<Uuid>>>>>,

    #[serde(default = "chrono::Utc::now")]
    pub created_at: Timestamp,

    #[serde(default = "chrono::Utc::now")]
    pub updated_at: Timestamp,
}

impl From<&FlowStep> for ArchivedFlowStep {
    fn from(flow_step: &FlowStep) -> Self {
        Self {
            node_id: flow_step.node_id,
            branch_id: flow_step.branch_id,
            flow_id: flow_step.flow_id,
            step_index: flow_step.step_index.clone(),
            id: flow_step.id,
            root_id: flow_step.root_id,
            node_ids: flow_step.node_ids.clone(),
            input_ids_by_node_id: flow_step.input_ids_by_node_id.clone(),
            output_ids_by_node_id: flow_step.output_ids_by_node_id.clone(),
            created_at: flow_step.created_at,
            updated_at: flow_step.updated_at,
        }
    }
}

partial_archived_flow_step!(PkArchivedFlowStep, id, branch_id, node_id, flow_id, step_index);

impl From<&FlowStep> for PkArchivedFlowStep {
    fn from(flow_step: &FlowStep) -> Self {
        Self {
            id: flow_step.id,
            branch_id: flow_step.branch_id,
            node_id: flow_step.node_id,
            flow_id: flow_step.flow_id,
            step_index: flow_step.step_index.clone(),
        }
    }
}
