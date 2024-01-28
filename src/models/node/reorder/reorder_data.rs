use crate::errors::NodecosmosError;
use crate::models::branch::branchable::Branchable;
use crate::models::node::reorder::ReorderParams;
use crate::models::node::{BaseNode, GetStructureNode, Node};
use crate::models::node_descendant::NodeDescendant;
use crate::models::utils::Pluckable;
use crate::utils::cloned_ref::ClonedRef;
use charybdis::model::AsNative;
use charybdis::operations::Find;
use charybdis::types::{Set, Uuid};
use scylla::CachingSession;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct ReorderData {
    pub node: Node,
    pub branch_id: Uuid,

    pub descendants: Vec<NodeDescendant>,
    pub descendant_ids: Vec<Uuid>,

    pub old_parent_id: Option<Uuid>,
    pub new_parent: BaseNode,

    pub old_ancestor_ids: Set<Uuid>,
    pub new_ancestor_ids: Set<Uuid>,

    pub removed_ancestor_ids: Vec<Uuid>,
    pub added_ancestor_ids: Vec<Uuid>,

    pub new_upper_sibling: Option<GetStructureNode>,
    pub new_lower_sibling: Option<GetStructureNode>,

    pub old_order_index: f64,
    pub new_order_index: f64,

    pub tree_root: GetStructureNode,
    pub tree_descendants: Vec<NodeDescendant>,
}

impl ReorderData {
    pub async fn from_params(params: &ReorderParams, db_session: &CachingSession) -> Result<Self, NodecosmosError> {
        let is_branched = params.id != params.branch_id;

        let node = Node::find_branched_or_original(&db_session, params.id, params.branch_id).await?;
        let descendants = if is_branched {
            node.descendants(&db_session).await?.try_collect().await?
        } else {
            node.branch_descendants(&db_session).await?
        };

        let descendant_ids = descendants.pluck_id();
        let old_parent_id = node.parent_id;

        let mut query_new_parent_node = Node {
            id: params.id,
            branch_id: params.branch_id,
            parent_id: Some(params.new_parent_id),
            ..Default::default()
        };
        let new_parent = query_new_parent_node.branch_parent(&db_session).await?;

        if new_parent.is_none() {
            return Err(NodecosmosError::NotFound(format!(
                "Parent with id {} and branch_id {} not found",
                params.new_parent_id, params.branch_id
            )));
        }

        let new_parent = new_parent.unwrap().clone();
        let old_ancestor_ids = node.ancestor_ids.cloned_ref();
        let new_ancestor_ids = build_new_ancestor_ids(&new_parent);

        let removed_ancestor_ids: Vec<Uuid> = old_ancestor_ids
            .iter()
            .filter(|&id| !new_ancestor_ids.contains(id))
            .cloned()
            .collect();

        let added_ancestor_ids: Vec<Uuid> = new_ancestor_ids
            .iter()
            .filter(|&id| !old_ancestor_ids.contains(id))
            .cloned()
            .collect();

        let new_upper_sibling = init_sibling(&db_session, params.new_upper_sibling_id, params.branch_id).await?;
        let new_lower_sibling = init_sibling(&db_session, params.new_lower_sibling_id, params.branch_id).await?;

        let old_order_index = node.order_index;
        let new_order_index = build_new_index(&new_upper_sibling, &new_lower_sibling);

        // used for recovery in case of failure mid-reorder
        let tree_root = GetStructureNode {
            id: node.root_id,
            branch_id: node.branchise_id(node.root_id),
            ..Default::default()
        }
        .find_by_primary_key(&db_session)
        .await?;

        let tree_descendants = if is_branched {
            tree_root
                .as_native()
                .descendants(&db_session)
                .await?
                .try_collect()
                .await?
        } else {
            tree_root.as_native().branch_descendants(&db_session).await?
        };

        let data = ReorderData {
            node,
            branch_id: params.branch_id,
            descendants,
            descendant_ids,

            old_parent_id,
            new_parent,

            old_ancestor_ids,
            new_ancestor_ids,

            removed_ancestor_ids,
            added_ancestor_ids,

            new_upper_sibling,
            new_lower_sibling,

            old_order_index,
            new_order_index,

            tree_root,
            tree_descendants,
        };

        Ok(data)
    }

    pub fn parent_changed(&self) -> bool {
        if let Some(current_parent_id) = self.node.parent_id {
            return current_parent_id != self.new_parent.id;
        }

        false
    }
}

pub async fn init_sibling(
    db_session: &CachingSession,
    id: Option<Uuid>,
    branch_id: Uuid,
) -> Result<Option<GetStructureNode>, NodecosmosError> {
    if let Some(id) = id {
        let node = GetStructureNode::find_branched_or_original(&db_session, id, branch_id).await?;

        return Ok(Some(node));
    }

    Ok(None)
}

pub fn build_new_ancestor_ids(new_parent: &BaseNode) -> Set<Uuid> {
    let mut new_ancestors = new_parent.ancestor_ids.cloned_ref();

    new_ancestors.insert(new_parent.id);

    new_ancestors
}

pub fn build_new_index(
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
