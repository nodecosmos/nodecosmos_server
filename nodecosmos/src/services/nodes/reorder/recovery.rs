use crate::errors::NodecosmosError;
use crate::models::node::{Node, UpdateOrderNode};
use crate::models::node_descendant::NodeDescendant;
use crate::services::logger::{log_error, log_fatal, log_success, log_warning};
use crate::services::nodes::reorder::reorder_data::ReorderData;
use crate::services::resource_locker::ResourceLocker;
use charybdis::{CharybdisModelBatch, Delete, Update};
use scylla::CachingSession;
use std::path::Path;

pub const RECOVERY_DATA_DIR: &str = "tmp/recovery";

/// Problem with using scylla batches for reordering tree structure is that they are
/// basically unusable for large trees (1000s nodes). We get a timeout error.
/// So in order to reorder large trees we need to split the queries into chunks and
/// this means that we don't have atomicity of reorder guaranteed.
/// As a result, we need custom recovery logic in case of failure.
pub struct Recovery<'a> {
    pub reorder_data: &'a ReorderData,
    pub db_session: &'a CachingSession,
    pub resource_locker: &'a ResourceLocker,
}

impl<'a> Recovery<'a> {
    pub fn new(
        reorder_data: &'a ReorderData,
        db_session: &'a CachingSession,
        resource_locker: &'a ResourceLocker,
    ) -> Self {
        Self {
            reorder_data,
            db_session,
            resource_locker,
        }
    }

    // Recovers Tree from recovery data stored on disk.
    // This may be needed in case of connection loss during reorder.
    pub async fn recover_from_stored_data(
        db_session: &CachingSession,
        resource_locker: &ResourceLocker,
    ) {
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
            let recovery_data: ReorderData = serde_json::from_str(&serialized)
                .map_err(|err| {
                    log_fatal(format!(
                        "Error in deserializing recovery data from file {}: {}",
                        full_name.clone(),
                        err
                    ));
                })
                .unwrap();
            std::fs::remove_file(full_name.clone()).unwrap();

            let mut recovery = Recovery::new(&recovery_data, db_session, resource_locker);
            let _ = recovery.recover().await.map_err(|err| {
                log_fatal(format!(
                    "Error in recovery from file {}: {}",
                    full_name, err
                ))
            });
        }
    }

    pub async fn recover(&mut self) -> Result<(), NodecosmosError> {
        let recovery_started = std::time::Instant::now();
        log_warning(format!(
            "Recovery started for tree_root: {}",
            self.reorder_data.tree_root.id
        ));

        let res = self.execute_recovery().await;

        match res {
            Ok(_) => {
                log_success(format!(
                    "Recovery finished for tree_root: {} in {:?}\n\n",
                    self.reorder_data.tree_root.id,
                    recovery_started.elapsed()
                ));

                self.resource_locker
                    .unlock_resource(&self.reorder_data.tree_root.id.to_string())
                    .await?;
            }
            Err(err) => {
                self.serialize_and_store_to_disk();

                log_error(format!("{}", err));
            }
        }

        Ok(())
    }

    async fn execute_recovery(&mut self) -> Result<(), NodecosmosError> {
        self.delete_tree().await?;
        self.restore_tree().await?;
        self.restore_node_order().await?;
        self.remove_new_ancestor_ids().await?;
        self.append_old_ancestor_ids().await?;

        Ok(())
    }

    async fn delete_tree(&mut self) -> Result<(), NodecosmosError> {
        NodeDescendant {
            root_id: self.reorder_data.tree_root.id,
            ..Default::default()
        }
        .delete_by_partition_key(self.db_session)
        .await
        .map_err(|err| {
            log_error(format!("delete_tree: {}", err));
            return err;
        })?;

        Ok(())
    }

    async fn restore_tree(&mut self) -> Result<(), NodecosmosError> {
        let now = std::time::Instant::now();

        CharybdisModelBatch::chunked_insert(
            self.db_session,
            &self.reorder_data.tree_descendants,
            100,
        )
        .await
        .map_err(|err| {
            log_error(format!("restore_tree_descendants: {}", err));
            return err;
        })?;

        log_warning(format!(
            "Tree descendants stored after: {:?}",
            now.elapsed()
        ));

        Ok(())
    }

    async fn restore_node_order(&mut self) -> Result<(), NodecosmosError> {
        let update_order_node = UpdateOrderNode {
            id: self.reorder_data.node.id,
            parent_id: Some(self.reorder_data.old_parent.id),
            order_index: Some(self.reorder_data.old_order_index),
        };

        update_order_node.update(&self.db_session).await?;

        Ok(())
    }

    async fn remove_new_ancestor_ids(&mut self) -> Result<(), NodecosmosError> {
        let mut node_and_descendant_ids = vec![self.reorder_data.node.id];
        node_and_descendant_ids.extend(self.reorder_data.descendant_ids.clone());

        for chunk in node_and_descendant_ids.chunks(100) {
            let mut batch = CharybdisModelBatch::new();

            for id in chunk {
                batch
                    .append_statement(
                        Node::PULL_FROM_ANCESTOR_IDS_QUERY,
                        (&self.reorder_data.new_node_ancestor_ids, id),
                    )
                    .map_err(|err| {
                        log_error(format!(
                            "append_statement for remove_new_ancestor_ids: {}",
                            err
                        ));

                        return err;
                    })?;
            }

            batch.execute(self.db_session).await.map_err(|err| {
                log_error(format!(
                    "removing new ancestor ids from node and its descendants: {}",
                    err
                ));

                return err;
            })?;
        }

        Ok(())
    }

    async fn append_old_ancestor_ids(&mut self) -> Result<(), NodecosmosError> {
        let mut node_and_descendant_ids = vec![self.reorder_data.node.id];
        node_and_descendant_ids.extend(self.reorder_data.descendant_ids.clone());

        for chunk in node_and_descendant_ids.chunks(100) {
            let mut batch = CharybdisModelBatch::new();

            for id in chunk {
                batch
                    .append_statement(
                        Node::PUSH_TO_ANCESTOR_IDS_QUERY,
                        (&self.reorder_data.old_node_ancestor_ids, id),
                    )
                    .map_err(|err| {
                        log_error(format!(
                            "append_statement for append_old_ancestor_ids: {}",
                            err
                        ));

                        return err;
                    })?;
            }

            batch.execute(self.db_session).await.map_err(|err| {
                log_error(format!(
                    "adding old ancestor ids to node and its descendants: {}",
                    err
                ));

                return err;
            })?;
        }

        Ok(())
    }

    fn serialize_and_store_to_disk(&mut self) {
        let serialized = serde_json::to_string(&self.reorder_data).unwrap();
        let filename = format!("recovery_data_{}.json", self.reorder_data.tree_root.id);
        let path = format!("{}/{}", RECOVERY_DATA_DIR, filename);
        let path_buf = Path::new(&path);

        // We only preserve the first recovery data file
        // as structure might be compromised later on if
        // user try to reorder again.
        if path_buf.exists() {
            log_error(format!("Recovery data file already exists: {}", path));

            return;
        }

        let res = std::fs::write(path.clone(), serialized);
        match res {
            Ok(_) => log_warning(format!("Recovery data saved to file: {}", path)),
            Err(err) => log_fatal(format!("Error in saving recovery data: {}", err)),
        }
    }
}
