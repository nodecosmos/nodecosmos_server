pub mod authorization;
pub mod branchable;
pub mod conflict;
pub mod merge;
pub mod update;

use crate::errors::NodecosmosError;
use crate::models::node::sort::SortNodes;
use crate::models::node::{
    find_update_description_node, find_update_title_node, Node, UpdateDescriptionNode, UpdateTitleNode,
};
use crate::models::udts::Owner;
use crate::models::udts::{BranchReorderData, Conflict};
use charybdis::macros::charybdis_model;
use charybdis::operations::{Find, Update};
use charybdis::types::{Boolean, Frozen, List, Map, Set, Text, Uuid};
use scylla::CachingSession;
use serde::{Deserialize, Serialize};
use std::rc::Rc;

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

    #[serde(rename = "ownerId")]
    pub owner_id: Uuid,

    pub owner: Option<Frozen<Owner>>,

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
    pub reordered_nodes: Option<Set<Frozen<BranchReorderData>>>,

    // workflows
    #[serde(default, rename = "createdWorkflows")]
    pub created_workflows: Option<Set<Uuid>>,

    #[serde(default, rename = "deletedWorkflows")]
    pub deleted_workflows: Option<Set<Uuid>>,

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
    pub edited_io_titles: Option<Set<Uuid>>,

    #[serde(default, rename = "editedIODescriptions")]
    pub edited_io_descriptions: Option<Set<Uuid>>,

    // flow steps
    #[serde(default, rename = "createdFlowSteps")]
    pub created_flow_steps: Option<Set<Uuid>>,

    #[serde(default, rename = "deletedFlowSteps")]
    pub deleted_flow_steps: Option<Set<Uuid>>,

    #[serde(default, rename = "createdFlowStepInputsByNode")]
    pub created_flow_step_inputs_by_node: Option<Frozen<Map<Uuid, Frozen<Set<Uuid>>>>>,

    #[serde(default, rename = "deletedFlowStepInputsByNode")]
    pub deleted_flow_step_inputs_by_node: Option<Frozen<Map<Uuid, Frozen<Set<Uuid>>>>>,

    #[serde(default, rename = "createdFlowStepOutputsByNode")]
    pub created_flow_step_outputs_by_node: Option<Frozen<Map<Uuid, Frozen<Set<Uuid>>>>>,

    #[serde(default, rename = "deletedFlowStepOutputsByNode")]
    pub deleted_flow_step_outputs_by_node: Option<Frozen<Map<Uuid, Frozen<Set<Uuid>>>>>,

    pub conflict: Option<Frozen<Conflict>>,

    #[serde(skip)]
    #[charybdis(ignore)]
    pub node: Option<Node>,
}

impl Branch {
    pub async fn node(&mut self, db_session: &CachingSession) -> Result<Option<&Node>, NodecosmosError> {
        if self.node.is_none() {
            let node = Node::find_by_primary_key_value(db_session, (self.node_id, self.node_id)).await?;

            self.node = Some(node);
        }

        Ok(self.node.as_ref())
    }

    pub async fn created_nodes(&self, db_session: &CachingSession) -> Result<Option<Vec<Node>>, NodecosmosError> {
        if let Some(created_nodes) = &self.created_nodes {
            let nodes = Node::find_branch_nodes(db_session, self.id, &created_nodes).await?;

            return Ok(Some(nodes));
        }

        Ok(None)
    }

    pub async fn restored_nodes(&self, db_session: &CachingSession) -> Result<Option<Vec<Node>>, NodecosmosError> {
        if let Some(restored_nodes) = &self.restored_nodes {
            let mut nodes = Node::find_branch_nodes(db_session, self.id, &restored_nodes).await?;

            nodes.sort_by_depth();

            return Ok(Some(nodes));
        }

        Ok(None)
    }

    pub async fn edited_title_nodes(
        &self,
        db_session: &CachingSession,
    ) -> Result<Option<Vec<UpdateTitleNode>>, NodecosmosError> {
        if let Some(edited_node_titles) = &self.edited_node_titles {
            let nodes = find_update_title_node!(db_session, "branch_id = ? AND id IN ?", (self.id, edited_node_titles))
                .await?
                .try_collect()
                .await?;

            return Ok(Some(nodes));
        }

        Ok(None)
    }

    pub async fn edited_description_nodes(
        &self,
        db_session: &CachingSession,
    ) -> Result<Option<Vec<UpdateDescriptionNode>>, NodecosmosError> {
        if let Some(edited_node_descriptions) = &self.edited_node_descriptions {
            let nodes = find_update_description_node!(
                db_session,
                "branch_id = ? AND id IN ?",
                (self.id, edited_node_descriptions)
            )
            .await?
            .try_collect()
            .await?;

            return Ok(Some(nodes));
        }

        Ok(None)
    }
}

partial_branch!(AuthBranch, id, owner_id, editor_ids, is_public);

partial_branch!(UpdateCreatedNodesBranch, id, created_nodes);

partial_branch!(UpdateDeletedNodesBranch, id, deleted_nodes);

partial_branch!(UpdateRestoredNodesBranch, id, restored_nodes);

partial_branch!(UpdateEditedNodeTitlesBranch, id, edited_node_titles);

partial_branch!(UpdateEditedNodeDescriptionsBranch, id, edited_node_descriptions);

partial_branch!(UpdateReorderedNodes, id, reordered_nodes);

partial_branch!(UpdateCreatedWorkflowsBranch, id, created_workflows);

partial_branch!(UpdateDeletedWorkflowsBranch, id, deleted_workflows);

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
