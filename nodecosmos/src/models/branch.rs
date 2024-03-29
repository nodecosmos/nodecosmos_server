pub mod conflict;
pub mod merge;
pub mod update;

use crate::errors::NodecosmosError;
use crate::models::description::ObjectType;
use crate::models::node::Node;
use crate::models::traits::{FindForBranchMerge, Id, ObjectId, Pluck};
use crate::models::udts::{BranchReorderData, Conflict};
use crate::models::udts::{Profile, TextChange};
use charybdis::macros::charybdis_model;
use charybdis::operations::Find;
use charybdis::types::{Boolean, Frozen, List, Map, Set, Text, Uuid};
use scylla::CachingSession;
use serde::{Deserialize, Serialize};
use std::cell::OnceCell;

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
pub struct Branch {
    pub id: Uuid,

    // where branch is created
    #[serde(rename = "nodeId")]
    pub node_id: Uuid,

    #[serde(rename = "rootId")]
    pub root_id: Uuid,

    pub title: Option<Text>,
    pub description: Option<Text>,

    #[serde(default = "BranchStatus::default")]
    pub status: Option<Text>,

    #[serde(rename = "ownerId")]
    pub owner_id: Uuid,

    pub owner: Option<Frozen<Profile>>,

    #[serde(rename = "editorIds")]
    pub editor_ids: Option<Set<Uuid>>,

    #[serde(rename = "isPublic")]
    pub is_public: Boolean,

    #[serde(rename = "isContributionRequest")]
    pub is_contribution_request: Option<Boolean>,

    // nodes
    #[serde(default, rename = "createdNodes")]
    pub created_nodes: Option<Set<Uuid>>,

    #[serde(default, rename = "restoredNodes")]
    pub restored_nodes: Option<Set<Uuid>>,

    #[serde(default, rename = "deletedNodes")]
    pub deleted_nodes: Option<Set<Uuid>>,

    #[serde(default, rename = "editedTitleNodes")]
    pub edited_title_nodes: Option<Set<Uuid>>,

    #[serde(default, rename = "editedDescriptionNodes")]
    pub edited_description_nodes: Option<Set<Uuid>>,

    #[serde(default, rename = "reorderedNodes")]
    pub reordered_nodes: Option<List<Frozen<BranchReorderData>>>,

    #[serde(default, rename = "editedWorkflowNodes")]
    pub edited_workflow_nodes: Option<Set<Uuid>>,

    /// node_id -> initial_input_ids
    #[serde(default, rename = "createdWorkflowInitialInputs")]
    pub created_workflow_initial_inputs: Option<Frozen<Map<Uuid, Frozen<List<Uuid>>>>>,

    /// node_id -> initial_input_ids
    #[serde(default, rename = "deletedWorkflowInitialInputs")]
    pub deleted_workflow_initial_inputs: Option<Frozen<Map<Uuid, Frozen<List<Uuid>>>>>,

    // flows
    #[serde(default, rename = "createdFlows")]
    pub created_flows: Option<Set<Uuid>>,

    #[serde(default, rename = "deletedFlows")]
    pub deleted_flows: Option<Set<Uuid>>,

    #[serde(default, rename = "restoredFlows")]
    pub restored_flows: Option<Set<Uuid>>,

    #[serde(default, rename = "editedTitleFlows")]
    pub edited_title_flows: Option<Set<Uuid>>,

    #[serde(default, rename = "editedDescriptionFlows")]
    pub edited_description_flows: Option<Set<Uuid>>,

    // flow steps
    #[serde(default, rename = "createdFlowSteps")]
    pub created_flow_steps: Option<Set<Uuid>>,

    #[serde(default, rename = "deletedFlowSteps")]
    pub deleted_flow_steps: Option<Set<Uuid>>,

    #[serde(default, rename = "restoredFlowSteps")]
    pub restored_flow_steps: Option<Set<Uuid>>,

    #[serde(default, rename = "editedDescriptionFlowSteps")]
    pub edited_description_flow_steps: Option<Set<Uuid>>,

    /// flow_step_id -> node_id
    #[serde(default, rename = "createdFlowStepNodes")]
    pub created_flow_step_nodes: Option<Frozen<Map<Uuid, Frozen<Set<Uuid>>>>>,

    /// flow_step_id -> node_id
    #[serde(default, rename = "deletedFlowStepNodes")]
    pub deleted_flow_step_nodes: Option<Frozen<Map<Uuid, Frozen<Set<Uuid>>>>>,

    /// flow_step_id -> node_id -> io_id
    #[serde(default, rename = "createdFlowStepInputsByNode")]
    pub created_flow_step_inputs_by_node: Option<Frozen<Map<Uuid, Frozen<Map<Uuid, Frozen<Set<Uuid>>>>>>>,

    /// flow_step_id -> node_id -> io_id
    #[serde(default, rename = "deletedFlowStepInputsByNode")]
    pub deleted_flow_step_inputs_by_node: Option<Frozen<Map<Uuid, Frozen<Map<Uuid, Frozen<Set<Uuid>>>>>>>,

    /// flow_step_id -> node_id -> io_id
    #[serde(default, rename = "createdFlowStepOutputsByNode")]
    pub created_flow_step_outputs_by_node: Option<Frozen<Map<Uuid, Frozen<Map<Uuid, Frozen<Set<Uuid>>>>>>>,

    /// flow_step_id -> node_id -> io_id
    #[serde(default, rename = "deletedFlowStepOutputsByNode")]
    pub deleted_flow_step_outputs_by_node: Option<Frozen<Map<Uuid, Frozen<Map<Uuid, Frozen<Set<Uuid>>>>>>>,

    // ios
    #[serde(default, rename = "createdIos")]
    pub created_ios: Option<Set<Uuid>>,

    #[serde(default, rename = "deletedIos")]
    pub deleted_ios: Option<Set<Uuid>>,

    #[serde(default, rename = "restoredIos")]
    pub restored_ios: Option<Set<Uuid>>,

    #[serde(default, rename = "editedTitleIos")]
    pub edited_title_ios: Option<Set<Uuid>>,

    #[serde(default, rename = "editedDescriptionIos")]
    pub edited_description_ios: Option<Set<Uuid>>,

    pub conflict: Option<Frozen<Conflict>>,

    #[serde(rename = "descriptionChangeByObject")]
    pub description_change_by_object: Option<Frozen<Map<Uuid, Frozen<TextChange>>>>,

    #[serde(rename = "titleChangeByObject")]
    pub title_change_by_object: Option<Frozen<Map<Uuid, Frozen<TextChange>>>>,

    #[serde(skip)]
    #[charybdis(ignore)]
    pub node: OnceCell<Node>,
}

impl Branch {
    pub async fn node(&self, db_session: &CachingSession) -> Result<&Node, NodecosmosError> {
        if let Some(node) = self.node.get() {
            return Ok(node);
        }

        let node = Node::find_by_primary_key_value(&(self.node_id, self.node_id))
            .execute(db_session)
            .await?;

        self.node
            .set(node)
            .map_err(|_| NodecosmosError::InternalServerError("Failed to set branch node".to_string()))?;

        Ok(self.node.get().unwrap())
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
    fn map_original_objects<'a, O: ObjectId + 'a>(
        &'a self,
        object_type: ObjectType,
        objects: Vec<O>,
    ) -> impl Iterator<Item = O> + 'a {
        let created_ids = self.created_ids(object_type);
        let deleted_ids = self.deleted_ids(object_type);

        objects.into_iter().filter(|object| {
            !created_ids
                .as_ref()
                .map_or(false, |created_nodes| created_nodes.contains(&object.object_id()))
                && !deleted_ids
                    .as_ref()
                    .map_or(false, |deleted_nodes| deleted_nodes.contains(&object.object_id()))
        })
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
                .map_or(false, |created_nodes| created_nodes.contains(&record.id()))
                && !deleted_ids
                    .as_ref()
                    .map_or(false, |deleted_nodes| deleted_nodes.contains(&record.id()))
        })
    }
}

partial_branch!(AuthBranch, id, owner_id, editor_ids, is_public, status);

partial_branch!(UpdateCreatedNodesBranch, id, created_nodes);

partial_branch!(UpdateDeletedNodesBranch, id, deleted_nodes);

partial_branch!(UpdateRestoredNodesBranch, id, restored_nodes);

partial_branch!(UpdateEditedTitleNodesBranch, id, edited_title_nodes);

partial_branch!(UpdateEditedDescriptionNodesBranch, id, edited_description_nodes);

partial_branch!(UpdateReorderedNodes, id, reordered_nodes);

partial_branch!(UpdateEditedNodeWorkflowsBranch, id, edited_workflow_nodes);

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

partial_branch!(UpdateCreatedIosBranch, id, created_ios);

partial_branch!(UpdateDeletedIosBranch, id, deleted_ios);

partial_branch!(UpdateRestoredIosBranch, id, restored_ios);

partial_branch!(UpdateEditedTitleIosBranch, id, edited_title_ios);

partial_branch!(UpdateEditedDescriptionIosBranch, id, edited_description_ios);

partial_branch!(UpdateCreatedFlowStepsBranch, id, created_flow_steps);

partial_branch!(UpdateDeletedFlowStepsBranch, id, deleted_flow_steps);

partial_branch!(
    UpdateEditedDescriptionFlowStepsBranch,
    id,
    edited_description_flow_steps
);

partial_branch!(UpdateRestoredFlowStepsBranch, id, restored_flow_steps);

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
