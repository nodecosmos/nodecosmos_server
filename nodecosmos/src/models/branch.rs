use charybdis::macros::charybdis_model;
use charybdis::types::{Boolean, Frozen, List, Map, Set, Text, Uuid};
use scylla::CachingSession;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use crate::errors::NodecosmosError;
use crate::models::node::Node;
use crate::models::traits::{Branchable, Id, ObjectType};
use crate::models::udts::{BranchReorderData, Conflict};
use crate::models::udts::{Profile, TextChange};

pub mod merge;
pub mod update;

#[derive(Copy, Clone, strum_macros::Display, strum_macros::EnumString)]
pub enum BranchStatus {
    Open,
    Merged,
    Recovered,
    RecoveryFailed,
    Closed,
}

impl BranchStatus {
    pub fn default() -> Option<String> {
        Some(BranchStatus::Open.to_string())
    }
}

#[charybdis_model(
    table_name = branches,
    partition_keys = [id],
    clustering_keys = [],
    table_options = r#"
        gc_grace_seconds = 432000
    "#,
)]
#[derive(Serialize, Deserialize, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Branch {
    pub id: Uuid,
    // where branch is created
    pub node_id: Uuid,
    pub root_id: Uuid,
    pub title: Option<Text>,
    pub description: Option<Text>,

    #[serde(default = "BranchStatus::default")]
    pub status: Option<Text>,
    pub owner_id: Uuid,
    pub owner: Option<Frozen<Profile>>,
    pub editor_ids: Option<Set<Uuid>>,
    pub viewer_ids: Option<Set<Uuid>>,
    pub is_public: Boolean,
    pub is_contribution_request: Option<Boolean>,
    // nodes
    pub created_nodes: Option<Set<Uuid>>,
    pub restored_nodes: Option<Set<Uuid>>,
    pub deleted_nodes: Option<Set<Uuid>>,
    pub edited_title_nodes: Option<Set<Uuid>>,
    pub edited_description_nodes: Option<Set<Uuid>>,
    pub reordered_nodes: Option<List<Frozen<BranchReorderData>>>,
    pub edited_nodes: Option<Set<Uuid>>,
    /// node_id -> initial_input_ids
    pub created_workflow_initial_inputs: Option<Map<Uuid, Frozen<List<Uuid>>>>,
    /// node_id -> initial_input_ids
    pub deleted_workflow_initial_inputs: Option<Map<Uuid, Frozen<List<Uuid>>>>,
    // flows
    pub created_flows: Option<Set<Uuid>>,
    pub deleted_flows: Option<Set<Uuid>>,
    pub restored_flows: Option<Set<Uuid>>,
    pub edited_title_flows: Option<Set<Uuid>>,
    pub edited_description_flows: Option<Set<Uuid>>,

    // flow steps
    pub created_flow_steps: Option<Set<Uuid>>,
    pub deleted_flow_steps: Option<Set<Uuid>>,
    pub restored_flow_steps: Option<Set<Uuid>>,
    /// Conflicting Flow Steps that were kept
    pub kept_flow_steps: Option<Set<Uuid>>,
    pub edited_description_flow_steps: Option<Set<Uuid>>,
    /// flow_step_id -> node_id
    pub created_flow_step_nodes: Option<Map<Uuid, Frozen<Set<Uuid>>>>,
    /// flow_step_id -> node_id
    pub deleted_flow_step_nodes: Option<Map<Uuid, Frozen<Set<Uuid>>>>,
    /// flow_step_id -> node_id -> io_id
    pub created_flow_step_inputs_by_node: Option<Map<Uuid, Frozen<Map<Uuid, Frozen<Set<Uuid>>>>>>,
    /// flow_step_id -> node_id -> io_id
    pub deleted_flow_step_inputs_by_node: Option<Map<Uuid, Frozen<Map<Uuid, Frozen<Set<Uuid>>>>>>,
    /// flow_step_id -> node_id -> io_id
    pub created_flow_step_outputs_by_node: Option<Map<Uuid, Frozen<Map<Uuid, Frozen<Set<Uuid>>>>>>,
    /// flow_step_id -> node_id -> io_id
    pub deleted_flow_step_outputs_by_node: Option<Map<Uuid, Frozen<Map<Uuid, Frozen<Set<Uuid>>>>>>,

    // ios
    pub created_ios: Option<Set<Uuid>>,
    pub deleted_ios: Option<Set<Uuid>>,
    pub restored_ios: Option<Set<Uuid>>,
    pub edited_title_ios: Option<Set<Uuid>>,
    pub edited_description_ios: Option<Set<Uuid>>,
    pub conflict: Option<Frozen<Conflict>>,
    pub description_change_by_object: Option<Frozen<Map<Uuid, Frozen<TextChange>>>>,
    pub title_change_by_object: Option<Frozen<Map<Uuid, Frozen<TextChange>>>>,

    #[serde(skip)]
    #[charybdis(ignore)]
    pub node: Option<Node>,
}

impl Branch {
    pub async fn contains_created_node(
        db_session: &CachingSession,
        id: Uuid,
        node_id: Uuid,
    ) -> Result<bool, NodecosmosError> {
        let branch = UpdateCreatedNodesBranch::find_by_id(id).execute(db_session).await?;

        Ok(branch
            .created_nodes
            .map_or(false, |created_nodes| created_nodes.contains(&node_id)))
    }

    pub async fn node(&mut self, db_session: &CachingSession) -> Result<&Node, NodecosmosError> {
        if self.node.is_none() {
            let node = Node::find_by_branch_id_and_id(self.original_id(), self.node_id)
                .execute(db_session)
                .await?;
            self.node = Some(node);
        }

        Ok(self.node.as_ref().unwrap())
    }

    pub fn all_edited_description_ids(&self) -> HashSet<Uuid> {
        let mut edited_object_ids = HashSet::new();

        if let Some(edited_description_nodes) = &self.edited_description_nodes {
            edited_object_ids.extend(edited_description_nodes.iter());
        }

        if let Some(edited_description_flows) = &self.edited_description_flows {
            edited_object_ids.extend(edited_description_flows.iter());
        }

        if let Some(edited_description_flow_steps) = &self.edited_description_flow_steps {
            edited_object_ids.extend(edited_description_flow_steps.iter());
        }

        if let Some(edited_description_ios) = &self.edited_description_ios {
            edited_object_ids.extend(edited_description_ios.iter());
        }

        edited_object_ids
    }

    pub fn all_deleted_object_ids(&self) -> HashSet<Uuid> {
        let mut deleted_object_ids = HashSet::new();

        if let Some(deleted_nodes) = &self.deleted_nodes {
            deleted_object_ids.extend(deleted_nodes.iter());
        }

        if let Some(deleted_flows) = &self.deleted_flows {
            deleted_object_ids.extend(deleted_flows.iter());
        }

        if let Some(deleted_flow_steps) = &self.deleted_flow_steps {
            deleted_object_ids.extend(deleted_flow_steps.iter());
        }

        if let Some(deleted_ios) = &self.deleted_ios {
            deleted_object_ids.extend(deleted_ios.iter());
        }

        deleted_object_ids
    }

    fn created_ids(&self, object_type: ObjectType) -> &Option<Set<Uuid>> {
        match object_type {
            ObjectType::Node => &self.created_nodes,
            ObjectType::Flow => &self.created_flows,
            ObjectType::FlowStep => &self.created_flow_steps,
            ObjectType::Io => &self.created_ios,
            ObjectType::Workflow => &None,
        }
    }

    fn deleted_ids(&self, object_type: ObjectType) -> &Option<Set<Uuid>> {
        match object_type {
            ObjectType::Node => &self.deleted_nodes,
            ObjectType::Flow => &self.deleted_flows,
            ObjectType::FlowStep => &self.deleted_flow_steps,
            ObjectType::Io => &self.deleted_ios,
            ObjectType::Workflow => &None,
        }
    }

    // records that are not created or deleted in the branch
    fn map_original_records<'a, R: Id + 'a>(
        &'a self,
        records: Vec<R>,
        object_type: ObjectType,
    ) -> impl Iterator<Item = R> + 'a {
        let created_ids = self.created_ids(object_type);
        let deleted_ids = self.deleted_ids(object_type);

        records.into_iter().filter(|record| {
            !created_ids
                .as_ref()
                .map_or(false, |created_ids| created_ids.contains(&record.id()))
                && !deleted_ids
                    .as_ref()
                    .map_or(false, |deleted_ids| deleted_ids.contains(&record.id()))
        })
    }
}

partial_branch!(AuthBranch, id, owner_id, editor_ids, viewer_ids, is_public, status);

partial_branch!(UpdateCreatedNodesBranch, id, created_nodes);

partial_branch!(UpdateDeletedNodesBranch, id, deleted_nodes);

partial_branch!(UpdateRestoredNodesBranch, id, restored_nodes);

partial_branch!(UpdateEditedTitleNodesBranch, id, edited_title_nodes);

partial_branch!(UpdateEditedDescriptionNodesBranch, id, edited_description_nodes);

partial_branch!(UpdateReorderedNodes, id, reordered_nodes);

partial_branch!(UpdateEditedNodesBranch, id, edited_nodes);

partial_branch!(
    UpdateCreatedWorkflowInitialInputsBranch,
    id,
    created_workflow_initial_inputs
);

partial_branch!(
    UpdateDeletedWorkflowInitialInputsBranch,
    id,
    deleted_workflow_initial_inputs
);

partial_branch!(UpdateCreatedFlowsBranch, id, created_flows);

partial_branch!(UpdateDeletedFlowsBranch, id, deleted_flows);

partial_branch!(UpdateRestoredFlowsBranch, id, restored_flows);

partial_branch!(UpdateEditedFlowTitleBranch, id, edited_title_flows);

partial_branch!(UpdateEditedFlowDescriptionBranch, id, edited_description_flows);

partial_branch!(UpdateCreatedFlowStepsBranch, id, created_flow_steps);

partial_branch!(UpdateDeletedFlowStepsBranch, id, deleted_flow_steps);

partial_branch!(
    UpdateEditedDescriptionFlowStepsBranch,
    id,
    edited_description_flow_steps
);

partial_branch!(UpdateRestoredFlowStepsBranch, id, restored_flow_steps);

partial_branch!(UpdateKeptFlowStepsBranch, id, kept_flow_steps);

partial_branch!(UpdateCreatedFlowStepNodesBranch, id, created_flow_step_nodes);

partial_branch!(UpdateDeletedFlowStepNodesBranch, id, deleted_flow_step_nodes);

partial_branch!(
    UpdateCreatedFlowStepInputsByNodeBranch,
    id,
    created_flow_step_inputs_by_node
);

partial_branch!(
    UpdateDeletedFlowStepInputsByNodeBranch,
    id,
    deleted_flow_step_inputs_by_node
);

partial_branch!(
    UpdateCreatedFlowStepOutputsByNodeBranch,
    id,
    created_flow_step_outputs_by_node
);

partial_branch!(
    UpdateDeletedFlowStepOutputsByNodeBranch,
    id,
    deleted_flow_step_outputs_by_node
);

partial_branch!(UpdateCreatedIosBranch, id, created_ios);

partial_branch!(UpdateDeletedIosBranch, id, deleted_ios);

partial_branch!(UpdateRestoredIosBranch, id, restored_ios);

partial_branch!(UpdateEditedTitleIosBranch, id, edited_title_ios);

partial_branch!(UpdateEditedDescriptionIosBranch, id, edited_description_ios);
