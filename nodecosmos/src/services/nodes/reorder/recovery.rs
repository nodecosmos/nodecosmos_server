use crate::errors::NodecosmosError;
use crate::models::node::{ReorderNode, UpdateNodeAncestorIds};
use crate::models::node_descendant::NodeDescendant;
use crate::services::logger::{log_error, log_fatal, log_success, log_warning};
use charybdis::{CharybdisModelBatch, Delete, Uuid};
use scylla::CachingSession;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

type ChildNodesByParentId<'a> = HashMap<Uuid, Vec<&'a NodeDescendant>>;

/// Problem with using scylla batches for complete tree structure is that they are
/// basically unusable for large trees (1000s nodes). We get a timeout error.
/// In order to allow reordering for large nodes, we have logic outside batches.
/// This means that we don't have atomicity of reorder queries guaranteed.
/// As a result, we need custom recovery logic in case of failure.
pub async fn recover_from_root(
    root: &ReorderNode,
    root_descendants: &Vec<NodeDescendant>,
    db_session: &CachingSession,
) {
    log_warning(format!("Recovery started for root: {}", root.id));

    let res = delete_tree(root, db_session).await.map_err(|err| {
        log_error(format!("Recovery Error in deleting tree: {}", err));
    });

    if res.is_err() {
        serialize_and_store_to_disk(root, root_descendants);
        return;
    }

    let now = std::time::Instant::now();

    let res = CharybdisModelBatch::chunked_insert(db_session, &root_descendants, 100)
        .await
        .map_err(|err| {
            log_error(format!(
                "Recovery Error in inserting root descendants: {}",
                err
            ));
            return err;
        });

    if res.is_err() {
        serialize_and_store_to_disk(root, root_descendants);
        return;
    }
    log_warning(format!("Descendant stored after: {:?}", now.elapsed()));

    let mut child_nodes_by_parent_id = ChildNodesByParentId::new();
    for descendant in root_descendants {
        child_nodes_by_parent_id
            .entry(descendant.parent_id.unwrap_or_default())
            .or_insert_with(Vec::new)
            .push(&descendant);
    }

    log_warning(format!("Rebuilding Tree Started for root: {}", root.id));
    let now = std::time::Instant::now();
    let res = rebuild_tree(root, &child_nodes_by_parent_id, db_session).await;
    match res {
        Ok(_) => {
            log_success(format!("Recovery successful for root: {}", root.id));
            log_success(format!("rebuild_tree took: {:?}", now.elapsed()));
        }
        Err(err) => {
            log_error(format!(
                "Recovery Error in recovering tree structure: {}",
                err
            ));
            serialize_and_store_to_disk(root, root_descendants);
        }
    }
}

async fn delete_tree(
    root: &ReorderNode,
    db_session: &CachingSession,
) -> Result<(), NodecosmosError> {
    NodeDescendant {
        root_id: root.id,
        ..Default::default()
    }
    .delete_by_partition_key(db_session)
    .await?;

    Ok(())
}

async fn rebuild_tree(
    root: &ReorderNode,
    child_nodes_by_parent_id: &ChildNodesByParentId<'_>,
    db_session: &CachingSession,
) -> Result<(), NodecosmosError> {
    // Each item in the stack is a tuple of a node ID and its ancestor IDs
    let mut stack = vec![(root.id, vec![])];
    let empty_child_nodes_vec = vec![];

    let mut update_ancestor_nodes = vec![];
    let mut insert_descendant_nodes = vec![];

    while let Some((parent_id, ancestor_ids)) = stack.pop() {
        let child_nodes = child_nodes_by_parent_id
            .get(&parent_id)
            .unwrap_or(&empty_child_nodes_vec);

        for node in child_nodes {
            let mut current_ancestor_ids = ancestor_ids.clone();
            current_ancestor_ids.push(parent_id);

            push_update_ancestor_node(
                &mut update_ancestor_nodes,
                node,
                parent_id,
                &current_ancestor_ids,
            );

            push_insert_descendants_nodes(
                &mut insert_descendant_nodes,
                node,
                &current_ancestor_ids,
            );

            stack.push((node.id, current_ancestor_ids));
        }
    }

    CharybdisModelBatch::chunked_update(db_session, &insert_descendant_nodes, 100)
        .await
        .map_err(|err| {
            log_error(format!("Recovery Error in saving descendants: {}", err));
            return err;
        })?;

    CharybdisModelBatch::chunked_update(db_session, &update_ancestor_nodes, 100)
        .await
        .map_err(|err| {
            log_error(format!("Recovery Error in updating ancestors: {}", err));
            return err;
        })?;

    Ok(())
}

fn push_update_ancestor_node(
    vec: &mut Vec<UpdateNodeAncestorIds>,
    node: &NodeDescendant,
    parent_id: Uuid,
    ancestor_ids: &Vec<Uuid>,
) {
    let update_anc_node = UpdateNodeAncestorIds {
        id: node.id,
        parent_id: Some(parent_id),
        ancestor_ids: Some(ancestor_ids.clone()),
    };

    vec.push(update_anc_node);
}

fn push_insert_descendants_nodes(
    vec: &mut Vec<NodeDescendant>,
    node: &NodeDescendant,
    ancestor_ids: &Vec<Uuid>,
) {
    for ancestor_id in ancestor_ids {
        let node_descendant = NodeDescendant {
            root_id: node.root_id,
            node_id: *ancestor_id,
            id: node.id,
            order_index: node.order_index,
            parent_id: node.parent_id,
            title: node.title.clone(),
        };

        vec.push(node_descendant);
    }
}

/// Hopefully this is never needed, but in case of a full db connection failure mid reorder,
/// we need to save the recovery data to disk.
#[derive(Serialize, Deserialize)]
pub struct RecoveryData {
    pub root: ReorderNode,
    pub root_descendants: Vec<NodeDescendant>,
}

const RECOVERY_DATA_DIR: &str = "tmp/recovery";

pub fn serialize_and_store_to_disk(root: &ReorderNode, root_descendants: &Vec<NodeDescendant>) {
    let recovery_data = RecoveryData {
        root: ReorderNode {
            id: root.id,
            root_id: root.root_id,
            parent_id: root.parent_id,
            ancestor_ids: root.ancestor_ids.clone(),
            order_index: root.order_index,
        },
        root_descendants: root_descendants.clone(),
    };

    let serialized = serde_json::to_string(&recovery_data).unwrap();
    let file_name = format!("{}/recovery_data_{}.json", RECOVERY_DATA_DIR, root.id);
    let res = std::fs::write(file_name.clone(), serialized);
    match res {
        Ok(_) => log_warning(format!("Recovery data saved to file: {}", file_name)),
        Err(err) => log_fatal(format!("Error in saving recovery data: {}", err)),
    }
}

pub async fn recover_reorder_failures(db_session: &CachingSession) {
    let mut file_names = vec![];
    for entry in std::fs::read_dir(RECOVERY_DATA_DIR).unwrap() {
        let entry = entry.unwrap();
        let file_name = entry.file_name().into_string().unwrap();
        if file_name.starts_with("recovery_data_") {
            file_names.push(file_name);
        }
    }

    for file_name in file_names {
        let full_name = format!("{}/{}", RECOVERY_DATA_DIR, file_name);
        let serialized = std::fs::read_to_string(full_name.clone()).unwrap();
        let recovery_data: RecoveryData = serde_json::from_str(&serialized)
            .map_err(|err| {
                log_fatal(format!(
                    "Error in deserializing recovery data from file {}: {}",
                    full_name, err
                ));
            })
            .unwrap();
        std::fs::remove_file(full_name).unwrap();

        recover_from_root(
            &recovery_data.root,
            &recovery_data.root_descendants,
            db_session,
        )
        .await;
    }
}
