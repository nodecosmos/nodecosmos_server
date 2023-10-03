use crate::models::node::ReorderNode;
use crate::models::node_descendant::NodeDescendant;
use charybdis::Update;
use csv::Writer;
use scylla::CachingSession;

// in case of a failure, we need to save the recovery data
// as csv file, so that we can recover nodes from the reorder failure
pub(crate) async fn recover_root_node(
    root: &ReorderNode,
    root_descendants: &Vec<NodeDescendant>,
    db_session: &CachingSession,
) {
    let root_insert_res = root.update(db_session).await;

    match root_insert_res {
        Ok(_) => {
            let root_descendants_chunks = root_descendants.chunks(100);

            for root_descendant_chunk in root_descendants_chunks {
                let mut batch = charybdis::CharybdisModelBatch::new();

                for descendant in root_descendant_chunk {
                    let res = batch.append_create(descendant).map_err(|err| {
                        println!("Recovery Error in adding descendant to batch: {}", err);
                    });

                    if res.is_err() {
                        save_to_csv_file(root, root_descendants);
                        return;
                    }
                }

                let res = batch.execute(db_session).await.map_err(|err| {
                    println!("Recovery Error in saving descendants: {}", err);
                });

                if res.is_err() {
                    save_to_csv_file(root, root_descendants);
                    return;
                }
            }
        }
        Err(err) => {
            println!("Recovery Error in saving root: {}", err);

            save_to_csv_file(root, root_descendants)
        }
    }
}

/// Hopefully this is never needed, but in case of a full db connection failure mid reorder,
/// we need to save the recovery data
pub fn save_to_csv_file(root: &ReorderNode, root_descendants: &Vec<NodeDescendant>) {
    let path = format!("recovery_data_{}.csv", root.id);
    let mut wtr = Writer::from_path(path).expect("failed to create csv writer");

    let root_id = root.id;

    wtr.write_record(&["root_id", "id", "parentId", "title", "order"])
        .expect("failed to write header");

    wtr.write_record(&[
        root_id.to_string(),
        root_id.to_string(),
        "".to_string(),
        "root".to_string(),
        "0".to_string(),
    ])
    .expect("failed to write root node");

    for descendant in root_descendants {
        let descendant_id = descendant.id;
        let res = wtr.write_record(&[
            root_id.to_string(),
            descendant_id.to_string(),
            descendant.parent_id.unwrap_or(root_id).to_string(),
            descendant.title.clone().unwrap_or("".to_string()),
            descendant.order_index.unwrap_or(0.0).to_string(),
        ]);

        if res.is_err() {
            let error = format!(
                "failed to serialize descendant node with id {}: {}",
                descendant_id,
                res.err().unwrap()
            );

            println!("Recovery Error: {}", error)
        }
    }
}
