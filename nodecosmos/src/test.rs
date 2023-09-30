use js_sys::Reflect::define_property;
use js_sys::{Array, Object, Reflect};
use serde::{Deserialize, Serialize};
use serde_wasm_bindgen::{from_value, to_value};
use std::collections::HashMap;

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsValue;

const MARGIN_LEFT: f32 = 20.0;
// move children's edge lef
const EDGE_LENGTH: f32 = 35.0;
// length of edge (link)
const COMPLETE_Y_LENGTH: f32 = 50.05;

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize)]
struct Node {
    treeNodeId: Option<String>,
    treeParentId: Option<String>,
    treeUpperSiblingId: Option<String>,
    treeAncestorIds: Vec<String>,
    treeChildIds: Option<Vec<String>>,
    treeDescendantIds: Option<Vec<String>>,
    treeLastChildId: Option<String>,
    nodeId: String,
    persistentNodeId: Option<String>,
    rootId: String,
    isRoot: bool,
    isMounted: bool,
    isExpanded: bool,
    isSelected: bool,
    isEditing: bool,
    nestedLevel: u16,
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize)]
struct Position {
    x: f32,
    y: f32,
    xEnd: f32,
    yEnd: f32,
}

// Test project that calculates the position of nodes in the tree.
// However, for this use-case performance is better when the tree is calculated in vanilla JS.
// Reason is serialization and deserialization of data.
#[wasm_bindgen]
pub fn calculate_position(ordered_tree_node_ids: JsValue, tree_nodes: JsValue) -> JsValue {
    let ordered_tree_node_ids: Vec<String> = from_value(ordered_tree_node_ids).unwrap();
    let tree_nodes: HashMap<String, Node> = from_value(tree_nodes).unwrap();

    let mut current_positions_by_id: HashMap<String, Position> = HashMap::with_capacity(1000);

    for id in ordered_tree_node_ids {
        let node = tree_nodes.get(&id).unwrap();

        let null_id = "null".to_string();

        let parent_id = node.treeParentId.as_ref().unwrap_or(&null_id);
        let parent_position = current_positions_by_id.get(parent_id);

        let upper_sibling_id = node.treeUpperSiblingId.as_ref().unwrap_or(&null_id);
        let upper_sibling_position = current_positions_by_id.get(upper_sibling_id);

        let Position {
            x: parent_x,
            y: parent_y,
            ..
        } = parent_position.unwrap_or(&Position {
            x: 0.0,
            y: 0.0,
            xEnd: 0.0,
            yEnd: 0.0,
        });
        let upper_sibling_y_end = upper_sibling_position
            .unwrap_or(&Position {
                x: 0.0,
                y: 0.0,
                xEnd: 0.0,
                yEnd: 0.0,
            })
            .yEnd;

        let x = parent_x + MARGIN_LEFT + EDGE_LENGTH;
        let y = if upper_sibling_y_end == 0.0 {
            parent_y + COMPLETE_Y_LENGTH
        } else {
            upper_sibling_y_end + COMPLETE_Y_LENGTH
        };

        let x_end = x + EDGE_LENGTH;

        let position = Position {
            x,
            y,
            xEnd: x_end,
            yEnd: y,
        };

        if node.isMounted {
            node.treeAncestorIds.iter().for_each(|ancestor_id| {
                current_positions_by_id.get_mut(ancestor_id).unwrap().yEnd += COMPLETE_Y_LENGTH;
            });
        }

        current_positions_by_id.insert(id, position);
    }

    return serde_wasm_bindgen::to_value(&current_positions_by_id).unwrap();
}

#[wasm_bindgen]
pub fn sum(a: i32, b: i32) -> i32 {
    a + b
}
