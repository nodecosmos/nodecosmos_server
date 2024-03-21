pub mod conflict;
pub mod merge;
pub mod update;

use crate::errors::NodecosmosError;
use crate::models::node::context::Context;
use crate::models::node::sort::SortNodes;
use crate::models::node::{
    find_update_description_node, find_update_title_node, Node, PkNode, UpdateDescriptionNode, UpdateTitleNode,
};
use crate::models::traits::{Id, Pluck};
use crate::models::udts::{BranchReorderData, Conflict};
use crate::models::udts::{Profile, TextChange};
use charybdis::macros::charybdis_model;
use charybdis::operations::Find;
use charybdis::types::{Boolean, Frozen, List, Map, Set, Text, Uuid};
use scylla::CachingSession;
use serde::{Deserialize, Serialize};
use std::fmt;

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

impl fmt::Display for BranchStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BranchStatus::Open => {
                write!(f, "Open")
            }
            BranchStatus::Merged => write!(f, "Merged"),
            BranchStatus::Recovered => write!(f, "Recovered"),
            BranchStatus::RecoveryFailed => write!(f, "RecoveryFailed"),
            BranchStatus::Closed => write!(f, "Closed"),
        }
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

    #[serde(default, rename = "editedNodeTitles")]
    pub edited_node_titles: Option<Set<Uuid>>,

    #[serde(default, rename = "editedNodeDescriptions")]
    pub edited_node_descriptions: Option<Set<Uuid>>,

    #[serde(default, rename = "reorderedNodes")]
    pub reordered_nodes: Option<List<Frozen<BranchReorderData>>>,

    // workflows
    #[serde(default, rename = "createdWorkflows")]
    pub created_workflows: Option<Set<Uuid>>,

    #[serde(default, rename = "deletedWorkflows")]
    pub deleted_workflows: Option<Set<Uuid>>,

    #[serde(default, rename = "createdWorkflowInitialInputs")]
    pub created_workflow_initial_inputs: Option<Set<Uuid>>,

    #[serde(default, rename = "deletedWorkflowInitialInputs")]
    pub deleted_workflow_initial_inputs: Option<Set<Uuid>>,

    #[serde(default, rename = "editedWorkflowTitles")]
    pub edited_workflow_titles: Option<Set<Uuid>>,

    // flows
    #[serde(default, rename = "createdFlows")]
    pub created_flows: Option<Set<Uuid>>,

    #[serde(default, rename = "deletedFlows")]
    pub deleted_flows: Option<Set<Uuid>>,

    #[serde(default, rename = "editedFlowTitles")]
    pub edited_flow_titles: Option<Set<Uuid>>,

    #[serde(default, rename = "editedFlowDescriptions")]
    pub edited_flow_descriptions: Option<Set<Uuid>>,

    // ios
    #[serde(default, rename = "createdIOs")]
    pub created_ios: Option<Set<Uuid>>,

    #[serde(default, rename = "deletedIOs")]
    pub deleted_ios: Option<Set<Uuid>>,

    #[serde(default, rename = "editedIOTitles")]
    pub edited_io_titles: Option<Frozen<Set<Text>>>,

    #[serde(default, rename = "editedIODescriptions")]
    pub edited_io_descriptions: Option<Set<Uuid>>,

    // flow steps
    #[serde(default, rename = "createdFlowSteps")]
    pub created_flow_steps: Option<Set<Uuid>>,

    #[serde(default, rename = "deletedFlowSteps")]
    pub deleted_flow_steps: Option<Set<Uuid>>,

    #[serde(default, rename = "createdFlowStepNodes")]
    pub created_flow_step_nodes: Option<Set<Uuid>>,

    /// First Uuid is the flow_step_id, second Uuid is the node_id, third Uuid is the io_id.
    #[serde(default, rename = "createdFlowStepInputsByNode")]
    pub created_flow_step_inputs_by_node: Option<Frozen<Map<Uuid, Frozen<Map<Uuid, Frozen<Set<Uuid>>>>>>>,

    /// First Uuid is the flow_step_id, second Uuid is the node_id, third Uuid is the io_id.
    #[serde(default, rename = "deletedFlowStepInputsByNode")]
    pub deleted_flow_step_inputs_by_node: Option<Frozen<Map<Uuid, Frozen<Map<Uuid, Frozen<Set<Uuid>>>>>>>,

    /// First Uuid is the flow_step_id, second Uuid is the node_id, third Uuid is the io_id.
    #[serde(default, rename = "createdFlowStepOutputsByNode")]
    pub created_flow_step_outputs_by_node: Option<Frozen<Map<Uuid, Frozen<Map<Uuid, Frozen<Set<Uuid>>>>>>>,

    /// First Uuid is the flow_step_id, second Uuid is the node_id, third Uuid is the io_id.
    #[serde(default, rename = "deletedFlowStepOutputsByNode")]
    pub deleted_flow_step_outputs_by_node: Option<Frozen<Map<Uuid, Frozen<Map<Uuid, Frozen<Set<Uuid>>>>>>>,

    pub conflict: Option<Frozen<Conflict>>,

    #[serde(rename = "descriptionChangeByObject")]
    pub description_change_by_object: Option<Frozen<Map<Uuid, Frozen<TextChange>>>>,

    #[serde(rename = "titleChangeByObject")]
    pub title_change_by_object: Option<Frozen<Map<Uuid, Frozen<TextChange>>>>,

    #[serde(skip)]
    #[charybdis(ignore)]
    pub node: Option<Node>,

    #[serde(skip)]
    #[charybdis(ignore)]
    pub _created_nodes: Option<Vec<Node>>,

    #[serde(skip)]
    #[charybdis(ignore)]
    pub _restored_nodes: Option<Vec<Node>>,

    #[serde(skip)]
    #[charybdis(ignore)]
    pub _deleted_nodes: Option<Vec<Node>>,

    #[serde(skip)]
    #[charybdis(ignore)]
    pub _original_title_nodes: Option<Vec<UpdateTitleNode>>,

    #[serde(skip)]
    #[charybdis(ignore)]
    pub _edited_title_nodes: Option<Vec<UpdateTitleNode>>,

    #[serde(skip)]
    #[charybdis(ignore)]
    pub _original_description_nodes: Option<Vec<UpdateDescriptionNode>>,

    #[serde(skip)]
    #[charybdis(ignore)]
    pub _edited_description_nodes: Option<Vec<UpdateDescriptionNode>>,
}

impl Branch {
    pub async fn node(&mut self, session: &CachingSession) -> Result<&Node, NodecosmosError> {
        if self.node.is_none() {
            let node = Node::find_by_primary_key_value(&(self.node_id, self.node_id))
                .execute(session)
                .await?;

            self.node = Some(node);
        }

        Ok(self.node.as_ref().unwrap())
    }

    pub async fn created_nodes(&mut self, session: &CachingSession) -> Result<Option<Vec<Node>>, NodecosmosError> {
        if let (None, Some(created_node_ids)) = (&self._created_nodes, &self.created_nodes) {
            let mut created_nodes = Node::find_by_ids_and_branch_id(session, &created_node_ids, self.id).await?;

            created_nodes.sort_by_depth();

            self._created_nodes = Some(created_nodes);
        }

        Ok(self._created_nodes.clone())
    }

    pub async fn restored_nodes(&mut self, session: &CachingSession) -> Result<Option<Vec<Node>>, NodecosmosError> {
        if let (None, Some(restored_node_ids)) = (&self._restored_nodes, &self.restored_nodes) {
            let mut branched_nodes = Node::find_by_ids_and_branch_id(session, &restored_node_ids, self.id).await?;
            let already_restored_ids = PkNode::find_by_ids(session, &branched_nodes.pluck_id())
                .await?
                .pluck_id_set();

            branched_nodes.retain(|branched_node| !already_restored_ids.contains(&branched_node.id));

            branched_nodes.sort_by_depth();

            self._restored_nodes = Some(branched_nodes);
        }

        Ok(self._restored_nodes.clone())
    }

    pub async fn edited_title_nodes(
        &mut self,
        session: &CachingSession,
    ) -> Result<Option<Vec<UpdateTitleNode>>, NodecosmosError> {
        if let (None, Some(edited_node_titles)) = (&self._edited_title_nodes, &self.edited_node_titles) {
            let nodes = find_update_title_node!("branch_id = ? AND id IN ?", (self.id, edited_node_titles))
                .execute(session)
                .await?
                .try_collect()
                .await?;

            let edited_title_nodes = self
                .retain_branch_nodes(nodes)
                .map(|mut edited_title_node| {
                    edited_title_node.ctx = Context::Merge;
                    edited_title_node
                })
                .collect();

            self._edited_title_nodes = Some(edited_title_nodes);
        }

        Ok(self._edited_title_nodes.clone())
    }

    pub async fn edited_description_nodes(
        &mut self,
        session: &CachingSession,
    ) -> Result<Option<Vec<UpdateDescriptionNode>>, NodecosmosError> {
        if let (None, Some(edited_node_descriptions)) =
            (&self._edited_description_nodes, &self.edited_node_descriptions)
        {
            let nodes = find_update_description_node!("branch_id = ? AND id IN ?", (self.id, edited_node_descriptions))
                .execute(session)
                .await?
                .try_collect()
                .await?;

            self._edited_description_nodes = Some(self.retain_branch_nodes(nodes).collect());
        }

        Ok(self._edited_description_nodes.clone())
    }

    pub fn reordered_nodes_data(&self) -> Option<List<Frozen<&BranchReorderData>>> {
        if let Some(reordered_nodes) = &self.reordered_nodes {
            Some(
                reordered_nodes
                    .into_iter()
                    .filter(|reorder_data| {
                        !self
                            .deleted_nodes
                            .as_ref()
                            .map_or(false, |deleted_nodes| deleted_nodes.contains(&reorder_data.id))
                            && !self
                                .created_nodes
                                .as_ref()
                                .map_or(false, |created_nodes| created_nodes.contains(&reorder_data.id))
                    })
                    .collect(),
            )
        } else {
            None
        }
    }

    /// Retain only the nodes that are not created or deleted in the branch
    fn retain_branch_nodes<'a, N: Id + 'a>(&'a self, nodes: Vec<N>) -> impl Iterator<Item = N> + 'a {
        nodes.into_iter().filter(|node| {
            !self
                .created_nodes
                .as_ref()
                .map_or(false, |created_nodes| created_nodes.contains(&node.id()))
                && !self
                    .deleted_nodes
                    .as_ref()
                    .map_or(false, |deleted_nodes| deleted_nodes.contains(&node.id()))
        })
    }
}

partial_branch!(AuthBranch, id, owner_id, editor_ids, is_public, status);

partial_branch!(UpdateCreatedNodesBranch, id, created_nodes);

partial_branch!(UpdateDeletedNodesBranch, id, deleted_nodes);

partial_branch!(UpdateRestoredNodesBranch, id, restored_nodes);

partial_branch!(UpdateEditedNodeTitlesBranch, id, edited_node_titles);

partial_branch!(UpdateEditedNodeDescriptionsBranch, id, edited_node_descriptions);

partial_branch!(UpdateReorderedNodes, id, reordered_nodes);

partial_branch!(UpdateCreatedWorkflowsBranch, id, created_workflows);

partial_branch!(UpdateDeletedWorkflowsBranch, id, deleted_workflows);

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

partial_branch!(UpdateEditedWorkflowTitlesBranch, id, edited_workflow_titles);

partial_branch!(UpdateCreatedFlowsBranch, id, created_flows);

partial_branch!(UpdateDeletedFlowsBranch, id, deleted_flows);

partial_branch!(UpdateEditedFlowTitlesBranch, id, edited_flow_titles);

partial_branch!(UpdateEditedFlowDescriptionsBranch, id, edited_flow_descriptions);

partial_branch!(UpdateCreatedIOsBranch, id, created_ios);

partial_branch!(UpdateDeletedIOsBranch, id, deleted_ios);

partial_branch!(UpdateEditedIOTitlesBranch, id, edited_io_titles);

partial_branch!(UpdateEditedIODescriptionsBranch, id, edited_io_descriptions);

partial_branch!(UpdateCreatedFlowStepsBranch, id, created_flow_steps);

partial_branch!(UpdateDeletedFlowStepsBranch, id, deleted_flow_steps);

partial_branch!(UpdateCreatedFlowStepNodesBranch, id, created_flow_step_nodes);

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
