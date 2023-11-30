use crate::errors::NodecosmosError;
use crate::models::node::{GetStructureNode, Node};
use crate::models::node_descendant::NodeDescendant;
use crate::models::utils::Pluckable;

use crate::models::branch::branchable::Branchable;
use crate::models::node::reorder::ReorderParams;
use crate::utils::cloned_ref::ClonedRef;
use charybdis::operations::Find;
use charybdis::types::Uuid;
use scylla::CachingSession;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct ReorderData {
    pub node: Node,
    pub branch_id: Uuid,

    pub descendants: Vec<NodeDescendant>,
    pub descendant_ids: Vec<Uuid>,

    pub old_parent_id: Option<Uuid>,
    pub new_parent: GetStructureNode,

    pub old_ancestor_ids: Vec<Uuid>,
    pub new_ancestor_ids: Vec<Uuid>,

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
        let node = Node::find_by_id_and_branch_id(&db_session, params.id, params.branch_id).await?;

        let descendants =
            NodeDescendant::find_by_root_id_and_branch_id_and_node_id(&db_session, node.root_id, node.id, node.id)
                .await?
                .try_collect()
                .await?;
        let descendant_ids = descendants.pluck_id();

        let old_parent_id = node.parent_id;

        let new_parent = GetStructureNode {
            id: params.new_parent_id,
            ..Default::default()
        }
        .find_by_primary_key(&db_session)
        .await?;

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

        let new_upper_sibling = init_sibling(params.new_upper_sibling_id, &db_session).await?;
        let new_lower_sibling = init_sibling(params.new_lower_sibling_id, &db_session).await?;

        let old_order_index = node.order_index;
        let new_order_index = build_new_index(&new_upper_sibling, &new_lower_sibling);

        let tree_root = GetStructureNode {
            id: node.root_id,
            ..Default::default()
        }
        .find_by_primary_key(&db_session)
        .await?;

        let tree_descendants = NodeDescendant {
            root_id: tree_root.id,
            branch_id: params.branch_id,
            ..Default::default()
        }
        .find_by_partition_key(&db_session)
        .await?
        .try_collect()
        .await?;

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

impl Branchable for ReorderData {
    fn id(&self) -> Uuid {
        self.node.id
    }

    fn branch_id(&self) -> Uuid {
        self.branch_id
    }
}

pub async fn init_sibling(
    id: Option<Uuid>,
    db_session: &CachingSession,
) -> Result<Option<GetStructureNode>, NodecosmosError> {
    if let Some(id) = id {
        let node = GetStructureNode {
            id,
            ..Default::default()
        }
        .find_by_primary_key(&db_session)
        .await?;

        return Ok(Some(node));
    }

    Ok(None)
}

pub fn build_new_ancestor_ids(new_parent: &GetStructureNode) -> Vec<Uuid> {
    let mut new_ancestors = new_parent.ancestor_ids.cloned_ref();

    new_ancestors.push(new_parent.id);

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
