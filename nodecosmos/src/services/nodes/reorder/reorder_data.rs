use crate::errors::NodecosmosError;
use crate::models::node::{Node, ReorderNode, UpdateNodeOrder};
use crate::models::node_descendant::NodeDescendant;
use crate::services::nodes::reorder::ReorderParams;
use charybdis::Find;
use scylla::CachingSession;

pub(crate) async fn find_reorder_data(
    db_session: &CachingSession,
    params: &ReorderParams,
) -> Result<
    (
        ReorderNode,
        Vec<NodeDescendant>,
        Node,
        ReorderNode,
        Vec<NodeDescendant>,
    ),
    NodecosmosError,
> {
    let root_node = ReorderNode {
        id: params.root_id,
        ..Default::default()
    };
    let root_q = root_node.find_by_primary_key(&db_session);

    let rot_node_descendants = NodeDescendant {
        root_id: params.root_id,
        ..Default::default()
    };
    let current_root_descendants_q = rot_node_descendants.find_by_partition_key(&db_session);

    let node = Node {
        id: params.id,
        ..Default::default()
    };
    let node_q = node.find_by_primary_key(&db_session);

    let new_parent_node = ReorderNode {
        id: params.new_parent_id,
        ..Default::default()
    };
    let new_parent_q = new_parent_node.find_by_primary_key(&db_session);

    let descendant = NodeDescendant {
        root_id: params.id,
        ..Default::default()
    };
    let descendants_q = descendant.find_by_partition_key(&db_session);

    // Execute all queries in parallel
    let futures = futures::join!(
        root_q,
        current_root_descendants_q,
        node_q,
        new_parent_q,
        descendants_q
    );

    let root = futures.0?;
    let current_root_descendants = futures.1?;
    let node = futures.2?;
    let new_parent = futures.3?;
    let descendants = futures.4?;

    return Ok((
        root,
        current_root_descendants,
        node,
        new_parent,
        descendants,
    ));
}

const ORDER_CORRECTION: f64 = 0.000000001;
/// Calculate new order index for node, as we can not update other nodes order index
pub(crate) async fn calculate_new_index(
    params: &ReorderParams,
    db_session: &CachingSession,
) -> Result<f64, NodecosmosError> {
    let mut new_order_index = 0f64;

    if let Some(new_bottom_sibling_id) = params.new_bottom_sibling_id {
        let new_bottom_sibling = UpdateNodeOrder {
            id: new_bottom_sibling_id,
            ..Default::default()
        }
        .find_by_primary_key(&db_session)
        .await?;
        let bottom_sibling_order_index = new_bottom_sibling.order_index.unwrap_or_default();

        if bottom_sibling_order_index == 0f64 {
            new_order_index = -0.1;
        } else {
            new_order_index = bottom_sibling_order_index - ORDER_CORRECTION;
        }
    }

    Ok(new_order_index)
}
