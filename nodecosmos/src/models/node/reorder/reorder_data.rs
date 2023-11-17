use crate::errors::NodecosmosError;
use crate::models::node::{GetStructureNode, Node};
use crate::models::node_descendant::NodeDescendant;
use crate::models::utils::Pluckable;

use crate::models::node::reorder::ReorderParams;
use crate::utils::cloned_ref::ClonedRef;
use charybdis::operations::Find;
use charybdis::types::Uuid;
use scylla::CachingSession;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct ReorderData {
    pub node: Node,
    pub descendants: Vec<NodeDescendant>,
    pub descendant_ids: Vec<Uuid>,

    pub old_parent: GetStructureNode,
    pub new_parent: GetStructureNode,

    pub old_node_ancestor_ids: Vec<Uuid>,
    pub new_node_ancestor_ids: Vec<Uuid>,

    pub new_upper_sibling: Option<GetStructureNode>,
    pub new_bottom_sibling: Option<GetStructureNode>,

    pub old_order_index: f64,
    pub new_order_index: f64,

    pub tree_root: GetStructureNode,
    pub tree_descendants: Vec<NodeDescendant>,
}

impl ReorderData {
    pub async fn from_params(params: &ReorderParams, db_session: &CachingSession) -> Result<Self, NodecosmosError> {
        let node = Node {
            id: params.node_id,
            ..Default::default()
        }
        .find_by_primary_key(&db_session)
        .await?;

        let descendants = NodeDescendant::find_by_root_id_and_node_id(&db_session, node.root_id, node.id)
            .await?
            .try_collect()
            .await?;
        let descendant_ids = descendants.pluck_id();

        let old_parent = GetStructureNode {
            id: node.parent_id.unwrap_or_default(),
            ..Default::default()
        }
        .find_by_primary_key(&db_session)
        .await?;

        let new_parent = GetStructureNode {
            id: params.new_parent_id,
            ..Default::default()
        }
        .find_by_primary_key(&db_session)
        .await?;

        let old_node_ancestor_ids = node.ancestor_ids.cloned_ref();
        let new_node_ancestor_ids = build_new_ancestor_ids(&new_parent);

        let new_upper_sibling = init_sibling(params.new_upper_sibling_id, &db_session).await?;
        let new_bottom_sibling = init_sibling(params.new_bottom_sibling_id, &db_session).await?;

        let old_order_index = node.order_index;
        let new_order_index = build_new_index(&new_upper_sibling, &new_bottom_sibling);

        let tree_root = GetStructureNode {
            id: node.root_id,
            ..Default::default()
        }
        .find_by_primary_key(&db_session)
        .await?;

        let tree_descendants = NodeDescendant {
            root_id: tree_root.id,
            ..Default::default()
        }
        .find_by_partition_key(&db_session)
        .await?
        .try_collect()
        .await?;

        let data = ReorderData {
            node,
            descendants,
            descendant_ids,

            old_parent,
            new_parent,

            old_node_ancestor_ids,
            new_node_ancestor_ids,

            new_upper_sibling,
            new_bottom_sibling,

            old_order_index,
            new_order_index,

            tree_root,
            tree_descendants,
        };

        Ok(data)
    }

    pub fn is_parent_changed(&self) -> bool {
        if let Some(current_parent_id) = self.node.parent_id {
            return current_parent_id != self.new_parent.id;
        }

        false
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
    new_bottom_sibling: &Option<GetStructureNode>,
) -> f64 {
    if new_upper_sibling.is_none() && new_bottom_sibling.is_none() {
        return 0.0;
    }

    let upper_sibling_index = if let Some(new_upper_sibling) = new_upper_sibling {
        new_upper_sibling.order_index
    } else {
        0.0
    };

    let bottom_sibling_index = if let Some(new_bottom_sibling) = new_bottom_sibling {
        new_bottom_sibling.order_index
    } else {
        0.0
    };

    // If only the bottom sibling exists, return its order index minus 1
    if new_upper_sibling.is_none() {
        return bottom_sibling_index - 1.0;
    }

    // If only the upper sibling exists, return its order index plus 1
    if new_bottom_sibling.is_none() {
        return upper_sibling_index + 1.0;
    }

    // If both siblings exist, return the average of their order indices
    // TODO: This is not ideal, as it has limited precision.
    (upper_sibling_index + bottom_sibling_index) / 2.0
}
