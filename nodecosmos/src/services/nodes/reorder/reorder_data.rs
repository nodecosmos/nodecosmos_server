use crate::errors::NodecosmosError;
use crate::models::helpers::clone_ref::ClonedRef;
use crate::models::node::{Node, ReorderNode};
use crate::models::node_descendant::NodeDescendant;
use crate::services::nodes::reorder::ReorderParams;
use charybdis::{Find, Uuid};
use scylla::CachingSession;

#[derive(Debug)]
pub struct ReorderData {
    pub root: ReorderNode,
    pub root_descendants: Vec<NodeDescendant>,
    pub node: Node,
    pub new_parent: ReorderNode,
    pub descendants: Vec<NodeDescendant>,
    pub new_upper_sibling: Option<ReorderNode>,
    pub new_bottom_sibling: Option<ReorderNode>,
}

pub async fn find_reorder_data(
    db_session: &CachingSession,
    params: &ReorderParams,
) -> Result<ReorderData, NodecosmosError> {
    let node = Node {
        id: params.node_id,
        ..Default::default()
    }
    .find_by_primary_key(&db_session)
    .await?;

    let root = ReorderNode {
        id: node.root_id,
        ..Default::default()
    }
    .find_by_primary_key(&db_session)
    .await?;

    let root_descendants = NodeDescendant {
        root_id: node.root_id,
        ..Default::default()
    }
    .find_by_partition_key(&db_session)
    .await?;

    let new_parent = ReorderNode {
        id: params.new_parent_id,
        ..Default::default()
    }
    .find_by_primary_key(&db_session)
    .await?;

    let descendants =
        NodeDescendant::find_by_root_id_and_node_id(&db_session, node.root_id, node.id).await?;

    let new_upper_sibling = init_sibling(params.new_upper_sibling_id, &db_session).await?;
    let new_bottom_sibling = init_sibling(params.new_bottom_sibling_id, &db_session).await?;

    let data = ReorderData {
        root,
        root_descendants,
        node,
        new_parent,
        descendants,
        new_upper_sibling,
        new_bottom_sibling,
    };

    Ok(data)
}

async fn init_sibling(
    id: Option<Uuid>,
    db_session: &CachingSession,
) -> Result<Option<ReorderNode>, NodecosmosError> {
    if let Some(id) = id {
        let node = ReorderNode {
            id,
            ..Default::default()
        }
        .find_by_primary_key(&db_session)
        .await?;

        return Ok(Some(node));
    }

    Ok(None)
}

pub fn build_new_ancestor_ids(new_parent: &ReorderNode) -> Vec<Uuid> {
    let new_parent_ancestors = new_parent.ancestor_ids.cloned_ref();
    let mut new_ancestors = Vec::with_capacity(new_parent_ancestors.len() + 1);

    new_ancestors.extend(new_parent_ancestors);
    new_ancestors.push(new_parent.id);

    new_ancestors
}

pub async fn build_new_index(
    new_upper_sibling: &Option<ReorderNode>,
    new_bottom_sibling: &Option<ReorderNode>,
) -> Result<f64, NodecosmosError> {
    if new_upper_sibling.is_none() && new_bottom_sibling.is_none() {
        return Ok(0.0);
    }

    let upper_sibling_index = if let Some(new_upper_sibling) = new_upper_sibling {
        new_upper_sibling.order_index.unwrap_or_default()
    } else {
        0.0
    };

    let bottom_sibling_index = if let Some(new_bottom_sibling) = new_bottom_sibling {
        new_bottom_sibling.order_index.unwrap_or_default()
    } else {
        0.0
    };

    // If only the bottom sibling exists, return its order index minus 1
    if new_upper_sibling.is_none() {
        return Ok(bottom_sibling_index - 1.0);
    }

    // If only the upper sibling exists, return its order index plus 1
    if new_bottom_sibling.is_none() {
        return Ok(upper_sibling_index + 1.0);
    }

    // If both siblings exist, return the average of their order indices
    // TODO: This is not ideal, as it has limited precision.
    //  after around of 1000 reorders between 0 and 1 the correct decimal point will be lost.
    //  Let's consider fractional index values in the future
    Ok((upper_sibling_index + bottom_sibling_index) / 2.0)
}
