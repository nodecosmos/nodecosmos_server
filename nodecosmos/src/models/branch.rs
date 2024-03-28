pub mod conflict;
pub mod merge;
pub mod update;

use crate::errors::NodecosmosError;
use crate::models::description::{find_description, Description};
use crate::models::flow::{Flow, UpdateTitleFlow};
use crate::models::flow_step::FlowStep;
use crate::models::node::context::Context;
use crate::models::node::sort::SortNodes;
use crate::models::node::{find_update_title_node, Node, PkNode, UpdateTitleNode};
use crate::models::traits::{FindForBranchMerge, Id, ObjectId, Pluck};
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

    #[serde(default, rename = "editedTitleNodes")]
    pub edited_title_nodes: Option<Set<Uuid>>,

    #[serde(default, rename = "editedDescriptionNodes")]
    pub edited_description_nodes: Option<Set<Uuid>>,

    #[serde(default, rename = "reorderedNodes")]
    pub reordered_nodes: Option<List<Frozen<BranchReorderData>>>,

    #[serde(default, rename = "editedWorkflowNodes")]
    pub edited_workflow_nodes: Option<Set<Uuid>>,

    #[serde(default, rename = "createdWorkflowInitialInputs")]
    pub created_workflow_initial_inputs: Option<Set<Uuid>>,

    #[serde(default, rename = "deletedWorkflowInitialInputs")]
    pub deleted_workflow_initial_inputs: Option<Set<Uuid>>,

    // flows
    #[serde(default, rename = "createdFlows")]
    pub created_flows: Option<Set<Uuid>>,

    #[serde(default, rename = "deletedFlows")]
    pub deleted_flows: Option<Set<Uuid>>,

    #[serde(default, rename = "editedTitleFlows")]
    pub edited_title_flows: Option<Set<Uuid>>,

    #[serde(default, rename = "editedDescriptionFlows")]
    pub edited_description_flows: Option<Set<Uuid>>,

    // flow steps
    #[serde(default, rename = "createdFlowSteps")]
    pub created_flow_steps: Option<Set<Uuid>>,

    #[serde(default, rename = "deletedFlowSteps")]
    pub deleted_flow_steps: Option<Set<Uuid>>,

    #[serde(default, rename = "editedDescriptionFlowSteps")]
    pub edited_description_flow_steps: Option<Set<Uuid>>,

    /// FlowStepId -> NodeId
    #[serde(default, rename = "createdFlowStepNodes")]
    pub created_flow_step_nodes: Option<Frozen<Map<Uuid, Frozen<Set<Uuid>>>>>,

    /// FlowStepId -> NodeId
    #[serde(default, rename = "deletedFlowStepNodes")]
    pub deleted_flow_step_nodes: Option<Frozen<Map<Uuid, Frozen<Set<Uuid>>>>>,

    /// FlowStepId -> NodeId -> IOId
    #[serde(default, rename = "createdFlowStepInputsByNode")]
    pub created_flow_step_inputs_by_node: Option<Frozen<Map<Uuid, Frozen<Map<Uuid, Frozen<Set<Uuid>>>>>>>,

    /// FlowStepId -> NodeId -> IOId
    #[serde(default, rename = "deletedFlowStepInputsByNode")]
    pub deleted_flow_step_inputs_by_node: Option<Frozen<Map<Uuid, Frozen<Map<Uuid, Frozen<Set<Uuid>>>>>>>,

    /// FlowStepId -> NodeId -> IOId
    #[serde(default, rename = "createdFlowStepOutputsByNode")]
    pub created_flow_step_outputs_by_node: Option<Frozen<Map<Uuid, Frozen<Map<Uuid, Frozen<Set<Uuid>>>>>>>,

    /// FlowStepId -> NodeId -> IOId
    #[serde(default, rename = "deletedFlowStepOutputsByNode")]
    pub deleted_flow_step_outputs_by_node: Option<Frozen<Map<Uuid, Frozen<Map<Uuid, Frozen<Set<Uuid>>>>>>>,

    // ios
    #[serde(default, rename = "createdIos")]
    pub created_ios: Option<Set<Uuid>>,

    #[serde(default, rename = "deletedIos")]
    pub deleted_ios: Option<Set<Uuid>>,

    #[serde(default, rename = "editedTitleIos")]
    pub edited_title_ios: Option<Frozen<Set<Text>>>,

    #[serde(default, rename = "editedDescriptionIos")]
    pub edited_description_ios: Option<Set<Uuid>>,

    pub conflict: Option<Frozen<Conflict>>,

    #[serde(rename = "descriptionChangeByObject")]
    pub description_change_by_object: Option<Frozen<Map<Uuid, Frozen<TextChange>>>>,

    #[serde(rename = "titleChangeByObject")]
    pub title_change_by_object: Option<Frozen<Map<Uuid, Frozen<TextChange>>>>,

    #[serde(skip)]
    #[charybdis(ignore)]
    pub node: Option<Node>,
}

impl Branch {
    pub async fn node(&mut self, db_session: &CachingSession) -> Result<&Node, NodecosmosError> {
        if self.node.is_none() {
            let node = Node::find_by_primary_key_value(&(self.node_id, self.node_id))
                .execute(db_session)
                .await?;

            self.node = Some(node);
        }

        Ok(self.node.as_ref().unwrap())
    }

    pub async fn created_nodes(&mut self, db_session: &CachingSession) -> Result<Option<Vec<Node>>, NodecosmosError> {
        if let Some(created_node_ids) = &self.created_nodes {
            let mut created_nodes = Node::find_by_ids_and_branch_id(db_session, &created_node_ids, self.id).await?;

            created_nodes.sort_by_depth();

            return Ok(Some(created_nodes));
        }

        Ok(None)
    }

    pub async fn restored_nodes(&mut self, db_session: &CachingSession) -> Result<Option<Vec<Node>>, NodecosmosError> {
        if let Some(restored_node_ids) = &self.restored_nodes {
            let mut branched_nodes = Node::find_by_ids_and_branch_id(db_session, &restored_node_ids, self.id).await?;
            let already_restored_ids = PkNode::find_by_ids(db_session, &branched_nodes.pluck_id())
                .await?
                .pluck_id_set();

            branched_nodes.retain(|branched_node| !already_restored_ids.contains(&branched_node.id));

            branched_nodes.sort_by_depth();

            return Ok(Some(branched_nodes));
        }

        Ok(None)
    }

    pub async fn edited_title_nodes(
        &mut self,
        db_session: &CachingSession,
    ) -> Result<Option<Vec<UpdateTitleNode>>, NodecosmosError> {
        if let Some(edited_title_nodes) = &self.edited_title_nodes {
            let nodes = find_update_title_node!("branch_id = ? AND id IN ?", (self.id, edited_title_nodes))
                .execute(db_session)
                .await?
                .try_collect()
                .await?;

            let edited_title_nodes = self
                .map_original_nodes(nodes)
                .map(|mut edited_title_node| {
                    edited_title_node.ctx = Context::Merge;
                    edited_title_node
                })
                .collect();

            return Ok(Some(edited_title_nodes));
        }

        Ok(None)
    }

    pub async fn edited_node_descriptions(
        &mut self,
        db_session: &CachingSession,
    ) -> Result<Option<Vec<Description>>, NodecosmosError> {
        if let Some(edited_description_nodes) = &self.edited_description_nodes {
            let descriptions =
                find_description!("branch_id = ? AND object_id IN ?", (self.id, edited_description_nodes))
                    .execute(db_session)
                    .await?
                    .try_collect()
                    .await?;

            return Ok(Some(self.map_original_objects(descriptions).collect()));
        }

        Ok(None)
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

    pub async fn created_flows(&mut self, db_session: &CachingSession) -> Result<Option<Vec<Flow>>, NodecosmosError> {
        if let (Some(edited_workflow_node_ids), Some(created_flow_ids)) =
            (&self.edited_workflow_nodes, &self.created_flows)
        {
            let flows = Flow::find_by_node_ids_and_branch_id_and_ids(
                db_session,
                edited_workflow_node_ids,
                self.id,
                created_flow_ids,
            )
            .await?;

            return Ok(Some(flows));
        }

        Ok(None)
    }

    pub async fn deleted_flows(&mut self, db_session: &CachingSession) -> Result<Option<Vec<Flow>>, NodecosmosError> {
        if let (Some(edited_workflow_node_ids), Some(deleted_flow_ids)) =
            (&self.edited_workflow_nodes, &self.deleted_flows)
        {
            let flows = Flow::find_by_node_ids_and_branch_id_and_ids(
                db_session,
                edited_workflow_node_ids,
                self.id,
                deleted_flow_ids,
            )
            .await?;

            return Ok(Some(flows));
        }

        Ok(None)
    }

    pub async fn edited_title_flows(
        &mut self,
        db_session: &CachingSession,
    ) -> Result<Option<Vec<UpdateTitleFlow>>, NodecosmosError> {
        if let (Some(edited_workflow_node_ids), Some(edited_title_flows)) =
            (&self.edited_workflow_nodes, &self.edited_title_flows)
        {
            let flows = UpdateTitleFlow::find_by_node_ids_and_branch_id_and_ids(
                db_session,
                edited_workflow_node_ids,
                self.id,
                edited_title_flows,
            )
            .await?;

            return Ok(Some(flows));
        }

        Ok(None)
    }

    pub async fn edited_flow_descriptions(
        &mut self,
        db_session: &CachingSession,
    ) -> Result<Option<Vec<Description>>, NodecosmosError> {
        if let Some(edited_description_flow_ids) = &self.edited_description_flows {
            let descriptions = find_description!(
                "branch_id = ? AND object_id IN ?",
                (self.id, edited_description_flow_ids)
            )
            .execute(db_session)
            .await?
            .try_collect()
            .await?;

            return Ok(Some(self.map_original_objects(descriptions).collect()));
        }

        Ok(None)
    }

    pub async fn created_flow_steps(
        &mut self,
        db_session: &CachingSession,
    ) -> Result<Option<Vec<FlowStep>>, NodecosmosError> {
        if let (Some(edited_workflow_node_ids), Some(created_flow_step_ids)) =
            (&self.edited_workflow_nodes, &self.created_flow_steps)
        {
            let flow_steps = FlowStep::find_by_node_ids_and_branch_id_and_ids(
                db_session,
                edited_workflow_node_ids,
                self.id,
                created_flow_step_ids,
            )
            .await?;
            return Ok(Some(flow_steps));
        }
        Ok(None)
    }

    pub async fn deleted_flow_steps(
        &mut self,
        db_session: &CachingSession,
    ) -> Result<Option<Vec<FlowStep>>, NodecosmosError> {
        if let (Some(edited_workflow_node_ids), Some(deleted_flow_step_ids)) =
            (&self.edited_workflow_nodes, &self.deleted_flow_steps)
        {
            let flow_steps = FlowStep::find_by_node_ids_and_branch_id_and_ids(
                db_session,
                edited_workflow_node_ids,
                self.id,
                deleted_flow_step_ids,
            )
            .await?;
            return Ok(Some(flow_steps));
        }
        Ok(None)
    }

    pub async fn edited_flow_step_descriptions(
        &mut self,
        db_session: &CachingSession,
    ) -> Result<Option<Vec<Description>>, NodecosmosError> {
        if let Some(edited_description_flow_step_ids) = &self.edited_description_flow_steps {
            let descriptions = find_description!(
                "branch_id = ? AND object_id IN ?",
                (self.id, edited_description_flow_step_ids)
            )
            .execute(db_session)
            .await?
            .try_collect()
            .await?;

            return Ok(Some(self.map_original_objects(descriptions).collect()));
        }

        Ok(None)
    }

    fn map_original_objects<'a, O: ObjectId + 'a>(&'a self, objects: Vec<O>) -> impl Iterator<Item = O> + 'a {
        objects.into_iter().filter(|object| {
            !self
                .created_nodes
                .as_ref()
                .map_or(false, |created_nodes| created_nodes.contains(&object.object_id()))
                && !self
                    .deleted_nodes
                    .as_ref()
                    .map_or(false, |deleted_nodes| deleted_nodes.contains(&object.object_id()))
        })
    }

    /// Retain only the nodes that are not created or deleted in the branch
    fn map_original_nodes<'a, N: Id + 'a>(&'a self, nodes: Vec<N>) -> impl Iterator<Item = N> + 'a {
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

partial_branch!(UpdateEditedFlowTitleBranch, id, edited_title_flows);

partial_branch!(UpdateEditedFlowDescriptionBranch, id, edited_description_flows);

partial_branch!(UpdateCreatedIosBranch, id, created_ios);

partial_branch!(UpdateDeletedIosBranch, id, deleted_ios);

partial_branch!(UpdateEditedTitleIosBranch, id, edited_title_ios);

partial_branch!(UpdateEditedDescriptionIosBranch, id, edited_description_ios);

partial_branch!(UpdateCreatedFlowStepsBranch, id, created_flow_steps);

partial_branch!(UpdateDeletedFlowStepsBranch, id, deleted_flow_steps);

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
