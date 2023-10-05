use crate::models::node::{Node, ReorderNode, UpdateNodeAncestorIds};
use crate::models::node_descendant::NodeDescendant;
use crate::services::logger::{log_error, log_warning};
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

            recover_tree_structure(root, &child_nodes_by_parent_id, db_session).await;
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

async fn recover_tree_structure(
    root: &ReorderNode,
    child_nodes_by_parent_id: &ChildNodesByParentId<'_>,
    db_session: &CachingSession,
) {
    let mut parent_ids = vec![root.id];
    let mut current_ancestor_ids = vec![];

    let empty_child_nodes_vec = vec![];

    while let Some(parent_id) = parent_ids.pop() {
        let child_nodes = child_nodes_by_parent_id
            .get(&parent_id)
            .unwrap_or(&empty_child_nodes_vec);

        current_ancestor_ids.push(parent_id);

        for node in child_nodes {
            update_ancestors(node, current_ancestor_ids.clone(), db_session).await;
            append_to_ancestors(node, current_ancestor_ids.clone(), db_session).await;
            parent_ids.push(node.id);
        }
    }
}

async fn update_ancestors(
    node: &NodeDescendant,
    ancestor_ids: Vec<Uuid>,
    db_session: &CachingSession,
) {
    let res = UpdateNodeAncestorIds {
        id: node.id,
        ancestor_ids: Some(ancestor_ids),
    }
    .update(db_session)
    .await;

    if res.is_err() {
        log_error(format!(
            "Recovery Error in updating descendant ancestors: {}",
            res.err().unwrap()
        ));
    }
}

async fn append_to_ancestors(
    node: &NodeDescendant,
    ancestor_ids: Vec<Uuid>,
    db_session: &CachingSession,
) {
    let mut node = Node {
        id: node.id,
        title: node.title.clone(),
        parent_id: node.parent_id,
        ancestor_ids: Some(ancestor_ids),
        order_index: node.order_index,
        ..Default::default()
    };

    let res = node.append_to_ancestors(db_session).await;
    match res {
        Ok(_) => (),
        Err(err) => log_error(format!(
            "Recovery Error in appending descendant ancestors: {}",
            err
        )),
    }
}

/// Hopefully this is never needed, but in case of a full db connection failure mid reorder,
/// we need to save the recovery data to disk.
#[derive(Serialize, Deserialize)]
pub struct RecoveryData {
    pub root: ReorderNode,
    pub root_descendants: Vec<NodeDescendant>,
}

pub fn serialize_and_store_to_disk(root: &ReorderNode, root_descendants: &Vec<NodeDescendant>) {
    let root_descendants = root_descendants.iter().cloned().collect::<Vec<_>>();

    let recovery_data = RecoveryData {
        root: ReorderNode {
            id: root.id,
            parent_id: root.parent_id,
            ancestor_ids: root.ancestor_ids.clone(),
        },
        root_descendants,
    };

    let serialized = serde_json::to_string(&recovery_data).unwrap();
    let file_name = format!("recovery_data_{}.json", root.id);
    std::fs::write(file_name, serialized).unwrap();
}

pub async fn recover_reorder_failures(db_session: &CachingSession) {
    let mut file_names = vec![];
    for entry in std::fs::read_dir("tmp/recovery").unwrap() {
        let entry = entry.unwrap();
        let file_name = entry.file_name().into_string().unwrap();
        if file_name.starts_with("recovery_data_") {
            file_names.push(file_name);
        }
    }

    for file_name in file_names {
        log_warning(format!("Recovering from file: {}", file_name));
        let serialized = std::fs::read_to_string(file_name.clone()).unwrap();
        let recovery_data: RecoveryData = serde_json::from_str(&serialized).unwrap();
        std::fs::remove_file(file_name).unwrap();

        recover_from_root(
            &recovery_data.root,
            &recovery_data.root_descendants,
            db_session,
        )
        .await;
    }
}
