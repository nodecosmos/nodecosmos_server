use charybdis::types::{Set, Uuid};
use scylla::CachingSession;
use serde::{Deserialize, Serialize};

use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::node::reorder::ReorderParams;
use crate::models::node::{GetStructureNode, Node};
use crate::models::node_descendant::NodeDescendant;
use crate::models::traits::{Branchable, FindOrInsertBranched, ModelBranchParams, NodeBranchParams, Pluck};
use crate::models::traits::{Descendants, Parent};
use crate::models::traits::{FindBranchedOrOriginalNode, RefCloned};

#[derive(Serialize, Deserialize, Clone)]
pub struct ReorderData {
    pub node: Node,
    pub branch_id: Uuid,

    pub descendants: Vec<NodeDescendant>,
    pub descendant_ids: Vec<Uuid>,

    pub old_parent_id: Uuid,
    pub new_parent_id: Uuid,

    pub new_parent_editor_ids: Option<Set<Uuid>>,

    pub old_ancestor_ids: Set<Uuid>,
    pub new_ancestor_ids: Set<Uuid>,

    pub removed_ancestor_ids: Set<Uuid>,
    pub added_ancestor_ids: Set<Uuid>,

    pub new_upper_sibling_id: Option<Uuid>,
    pub new_lower_sibling_id: Option<Uuid>,

    pub new_upper_sibling: Option<GetStructureNode>,
    pub new_lower_sibling: Option<GetStructureNode>,

    pub old_order_index: f64,
    pub new_order_index: f64,
}

impl ReorderData {
    async fn find_descendants(
        db_session: &CachingSession,
        params: &ReorderParams,
        node: &Node,
    ) -> Result<Vec<NodeDescendant>, NodecosmosError> {
        return if params.is_branch() {
            node.branch_descendants(&db_session).await
        } else {
            node.descendants(&db_session)
                .await?
                .try_collect()
                .await
                .map_err(NodecosmosError::from)
        };
    }

    async fn find_new_parent(data: &RequestData, params: &ReorderParams) -> Result<Node, NodecosmosError> {
        let mut query_new_parent_node = Node {
            id: params.id,
            branch_id: params.branch_id,
            parent_id: Some(params.new_parent_id),
            root_id: params.root_id,
            ..Default::default()
        };
        let new_parent = query_new_parent_node.parent(data.db_session()).await?;

        match new_parent {
            Some(new_parent) => Ok(*new_parent.clone()),
            None => Err(NodecosmosError::NotFound(format!(
                "Parent with id {} and branch_id {} not found",
                params.new_parent_id, params.branch_id
            ))),
        }
    }

    async fn init_sibling(
        db_session: &CachingSession,
        params: NodeBranchParams,
    ) -> Result<Option<GetStructureNode>, NodecosmosError> {
        let node = GetStructureNode::find_branched_or_original(&db_session, params).await?;

        return Ok(Some(node));
    }

    fn build_new_ancestor_ids(new_parent: &Node) -> Set<Uuid> {
        let mut new_ancestors = new_parent.ancestor_ids.ref_cloned();

        new_ancestors.insert(new_parent.id);

        new_ancestors
    }

    fn build_new_index(
        new_upper_sibling: &Option<GetStructureNode>,
        new_lower_sibling: &Option<GetStructureNode>,
    ) -> f64 {
        if new_upper_sibling.is_none() && new_lower_sibling.is_none() {
            return 0.0;
        }

        let upper_sibling_index = if let Some(new_upper_sibling) = new_upper_sibling {
            new_upper_sibling.order_index
        } else {
            0.0
        };

        let lower_sibling_index = if let Some(new_lower_sibling) = new_lower_sibling {
            new_lower_sibling.order_index
        } else {
            0.0
        };

        // If only the bottom sibling exists, return its order index minus 1
        if new_upper_sibling.is_none() {
            return lower_sibling_index - 1.0;
        }

        // If only the upper sibling exists, return its order index plus 1
        if new_lower_sibling.is_none() {
            return upper_sibling_index + 1.0;
        }

        // If both siblings exist, return the average of their order indices
        // TODO: This is not ideal, as it has limited precision.
        (upper_sibling_index + lower_sibling_index) / 2.0
    }

    fn extract_removed_ancestor_ids(old_ancestor_ids: &Set<Uuid>, new_ancestor_ids: &Set<Uuid>) -> Set<Uuid> {
        old_ancestor_ids
            .iter()
            .filter(|&id| !new_ancestor_ids.contains(id))
            .cloned()
            .collect()
    }

    fn extract_added_ancestor_ids(old_ancestor_ids: &Set<Uuid>, new_ancestor_ids: &Set<Uuid>) -> Set<Uuid> {
        new_ancestor_ids
            .iter()
            .filter(|&id| !old_ancestor_ids.contains(id))
            .cloned()
            .collect()
    }

    pub async fn from_params(params: &ReorderParams, data: &RequestData) -> Result<Self, NodecosmosError> {
        let node = Node::find_or_insert_branched(
            data,
            ModelBranchParams {
                original_id: params.original_id(),
                branch_id: params.branch_id,
                node_id: params.id,
                id: params.id,
            },
        )
        .await?;
        node.preserve_branch_ancestors(data).await?;

        let descendants = Self::find_descendants(data.db_session(), params, &node).await?;
        let descendant_ids = descendants.pluck_id();
        let old_parent_id = match node.parent_id {
            Some(id) => id,
            None => return Err(NodecosmosError::BadRequest("Node has no parent".to_string())),
        };
        let new_parent = Self::find_new_parent(data, params).await?;
        let old_ancestor_ids = node.ancestor_ids.ref_cloned();
        let new_ancestor_ids = Self::build_new_ancestor_ids(&new_parent);
        let removed_ancestor_ids = Self::extract_removed_ancestor_ids(&old_ancestor_ids, &new_ancestor_ids);
        let added_ancestor_ids = Self::extract_added_ancestor_ids(&old_ancestor_ids, &new_ancestor_ids);
        let new_upper_sibling = if let Some(id) = params.new_upper_sibling_id {
            Self::init_sibling(
                data.db_session(),
                NodeBranchParams {
                    root_id: params.root_id,
                    branch_id: params.branch_id,
                    node_id: id,
                },
            )
            .await?
        } else {
            None
        };
        let new_lower_sibling = if let Some(id) = params.new_lower_sibling_id {
            Self::init_sibling(
                data.db_session(),
                NodeBranchParams {
                    root_id: params.root_id,
                    branch_id: params.branch_id,
                    node_id: id,
                },
            )
            .await?
        } else {
            None
        };
        let old_order_index = node.order_index;
        let new_order_index = match params.new_order_index {
            Some(index) => index,
            None => Self::build_new_index(&new_upper_sibling, &new_lower_sibling),
        };

        let data = ReorderData {
            node,
            branch_id: params.branch_id,
            descendants,
            descendant_ids,
            old_parent_id,
            new_parent_id: new_parent.id,
            new_parent_editor_ids: new_parent.editor_ids,
            old_ancestor_ids,
            new_ancestor_ids,
            removed_ancestor_ids,
            added_ancestor_ids,
            new_upper_sibling_id: params.new_upper_sibling_id,
            new_lower_sibling_id: params.new_lower_sibling_id,
            new_upper_sibling,
            new_lower_sibling,
            old_order_index,
            new_order_index,
        };

        Ok(data)
    }

    pub fn parent_changed(&self) -> bool {
        if let Some(current_parent_id) = self.node.parent_id {
            return current_parent_id != self.new_parent_id;
        }

        false
    }
}
