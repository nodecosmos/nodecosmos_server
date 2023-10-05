use crate::errors::NodecosmosError;
use crate::models::node::{Node, ReorderNode, UpdateNodeAncestorIds};
use crate::models::node_descendant::NodeDescendant;
use crate::services::logger::{log_error, log_fatal, log_success, log_warning};
use charybdis::{Serialize, Update, Uuid};
use scylla::CachingSession;
use serde::Deserialize;
use std::collections::HashMap;

type ChildNodesByParentId<'a> = HashMap<Uuid, Vec<&'a NodeDescendant>>;

/// Problem with using scylla batches for complete tree structure is that they are
/// basically unusable for large trees (1000s nodes). We get a timeout error.
/// In order to allow reordering for large nodes, we have logic outside batches.
/// This means that we don't have atomicity of reorder queries guaranteed.
/// As a result, we need custom recovery logic in case of failure.
pub(crate) async fn recover_from_root(
    root: &ReorderNode,
    root_descendants: &Vec<NodeDescendant>,
    db_session: &CachingSession,
) {
    let root_insert_res = root.update(db_session).await;
    let mut child_nodes_by_parent_id = ChildNodesByParentId::new();

    let res = delete_tree(root, root_descendants, db_session)
        .await
        .map_err(|err| {
            log_error(format!("Recovery Error in deleting tree: {}", err));
        });

    if res.is_err() {
        serialize_and_store_to_disk(root, root_descendants);
        return;
    }

    match root_insert_res {
        Ok(_) => {
            let root_descendants_chunks = root_descendants.chunks(100);

            for root_descendant_chunk in root_descendants_chunks {
                let mut batch = charybdis::CharybdisModelBatch::new();

                for descendant in root_descendant_chunk {
                    let res = batch.append_create(descendant).map_err(|err| {
                        log_error(format!(
                            "Recovery Error in adding descendant to batch: {}",
                            err
                        ));
                    });

                    if res.is_err() {
                        serialize_and_store_to_disk(root, root_descendants);
                        return;
                    }

                    let parent_id = descendant.parent_id.unwrap_or_default();
                    child_nodes_by_parent_id
                        .entry(parent_id)
                        .or_insert_with(Vec::new)
                        .push(&descendant);
                }

                let res = batch.execute(db_session).await.map_err(|err| {
                    log_error(format!("Recovery Error in saving descendants: {}", err));
                });

                if res.is_err() {
                    serialize_and_store_to_disk(root, root_descendants);
                    return;
                }
            }

            let res = rebuild_tree(root, &child_nodes_by_parent_id, db_session).await;
            match res {
                Ok(_) => {
                    log_success(format!("Recovery successful for root: {}", root.id));
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
        Err(err) => {
            log_error(format!(
                "Recovery Error in saving root: {}\n RootId: {}",
                err, root.id
            ));
            serialize_and_store_to_disk(root, root_descendants)
        }
    }
}

async fn delete_tree(
    root: &ReorderNode,
    root_descendants: &Vec<NodeDescendant>,
    db_session: &CachingSession,
) -> Result<(), NodecosmosError> {
    let root_node_descendant = NodeDescendant {
        id: root.id,
        ..Default::default()
    };

    delete_descendants(&root_node_descendant, db_session).await?;

    for descendant in root_descendants {
        delete_descendants(descendant, db_session).await?;
    }

    Ok(())
}

async fn delete_descendants(
    node: &NodeDescendant,
    db_session: &CachingSession,
) -> Result<(), NodecosmosError> {
    let descendants = Node {
        id: node.id,
        ..Default::default()
    }
    .descendants(db_session)
    .await?;
    let descendants_chunks = descendants.chunks(100);

    for descendant_chunk in descendants_chunks {
        let mut batch = charybdis::CharybdisModelBatch::new();
        batch.append_deletes(descendant_chunk.to_vec())?;

        batch.execute(db_session).await?;
    }

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

    while let Some((node_id, ancestor_ids)) = stack.pop() {
        let child_nodes = child_nodes_by_parent_id
            .get(&node_id)
            .unwrap_or(&empty_child_nodes_vec);

        for node in child_nodes {
            let mut current_ancestor_ids = ancestor_ids.clone();
            current_ancestor_ids.push(node_id);

            update_ancestors(node, Some(node_id), &current_ancestor_ids, db_session).await?;
            append_to_ancestors(node, &current_ancestor_ids, db_session).await?;

            stack.push((node.id, current_ancestor_ids));
        }
    }

    Ok(())
}

async fn update_ancestors(
    node: &NodeDescendant,
    parent_id: Option<Uuid>,
    ancestor_ids: &Vec<Uuid>,
    db_session: &CachingSession,
) -> Result<(), NodecosmosError> {
    UpdateNodeAncestorIds {
        id: node.id,
        parent_id,
        ancestor_ids: Some(ancestor_ids.clone()),
    }
    .update(db_session)
    .await
    .map_err(|err| {
        log_error(format!(
            "Recovery Error in updating descendant ancestors: {}",
            err
        ));

        err
    })?;

    Ok(())
}

async fn append_to_ancestors(
    node: &NodeDescendant,
    ancestor_ids: &Vec<Uuid>,
    db_session: &CachingSession,
) -> Result<(), NodecosmosError> {
    let mut node = Node {
        id: node.id,
        title: node.title.clone(),
        parent_id: node.parent_id,
        ancestor_ids: Some(ancestor_ids.clone()),
        order_index: node.order_index,
        ..Default::default()
    };

    node.append_to_ancestors(db_session).await.map_err(|err| {
        log_error(format!(
            "Recovery Error in appending descendant ancestors: {}",
            err
        ));

        err
    })?;

    Ok(())
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
            parent_id: root.parent_id,
            ancestor_ids: root.ancestor_ids.clone(),
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
