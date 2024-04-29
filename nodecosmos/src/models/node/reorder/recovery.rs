use std::fs::create_dir_all;
use std::path::Path;

use charybdis::batch::ModelBatch;
use charybdis::operations::Update;
use log::{error, info, warn};
use scylla::CachingSession;

use crate::errors::NodecosmosError;
use crate::models::node::reorder::data::ReorderData;
use crate::models::node::{Node, UpdateOrderNode};
use crate::models::node_descendant::NodeDescendant;
use crate::models::utils::file::read_file_names;
use crate::resources::resource_locker::ResourceLocker;

pub const RECOVERY_DATA_DIR: &str = "tmp/reorder-recovery";
pub const RECOVER_FILE_PREFIX: &str = "reorder_recovery_data";

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
    pub async fn recover_from_stored_data(db_session: &CachingSession, resource_locker: &ResourceLocker) {
        create_dir_all(RECOVERY_DATA_DIR).expect("Error in creating recovery data directory");
        let files = read_file_names(RECOVERY_DATA_DIR, RECOVER_FILE_PREFIX).await;

        for file in files {
            let serialized = std::fs::read_to_string(file.clone()).unwrap();
            let recovery_data: ReorderData = serde_json::from_str(&serialized)
                .map_err(|err| {
                    error!(
                        "Error in deserializing recovery data from file {}: {}",
                        file.clone(),
                        err
                    );
                })
                .expect("Error in deserializing recovery data");
            std::fs::remove_file(file.clone()).expect("Error in removing recovery data file");

            let mut recovery = Recovery::new(&recovery_data, db_session, resource_locker);
            let _ = recovery
                .recover()
                .await
                .map_err(|err| error!("Error in recovery from file {}: {}", file, err));
        }
    }

    pub async fn recover(&mut self) -> Result<(), NodecosmosError> {
        let recovery_started = std::time::Instant::now();
        warn!("Recovery started for tree_root: {}", self.reorder_data.tree_root.id);

        let res = self.execute_recovery().await;

        match res {
            Ok(_) => {
                info!(
                    "Recovery finished for tree_root: {} in {:?}\n\n",
                    self.reorder_data.tree_root.id,
                    recovery_started.elapsed()
                );

                self.resource_locker
                    .unlock_resource(self.reorder_data.tree_root.id, self.reorder_data.tree_root.branch_id)
                    .await?;
            }
            Err(err) => {
                self.serialize_and_store_to_disk();

                error!("recover: {}", err);

                return Err(NodecosmosError::FatalReorderError(err.to_string()));
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
        NodeDescendant::delete_by_root_id_and_branch_id(self.reorder_data.tree_root.id, self.reorder_data.branch_id)
            .execute(self.db_session)
            .await
            .map_err(|err| {
                error!("delete_by_root_id_and_branch_id: {}", err);
                return err;
            })?;

        Ok(())
    }

    async fn restore_tree(&mut self) -> Result<(), NodecosmosError> {
        let now = std::time::Instant::now();

        NodeDescendant::unlogged_batch()
            .chunked_insert(
                self.db_session,
                &self.reorder_data.tree_descendants,
                crate::constants::BATCH_CHUNK_SIZE,
            )
            .await
            .map_err(|err| {
                error!("restore_tree_descendants: {}", err);
                return err;
            })?;

        warn!("Tree descendants stored after: {:?}", now.elapsed());

        Ok(())
    }

    async fn restore_node_order(&mut self) -> Result<(), NodecosmosError> {
        let update_order_node = UpdateOrderNode {
            id: self.reorder_data.node.id,
            branch_id: self.reorder_data.branch_id,
            root_id: self.reorder_data.tree_root.id,
            parent_id: self.reorder_data.old_parent_id,
            order_index: self.reorder_data.old_order_index,
        };

        update_order_node.update().execute(&self.db_session).await?;

        Ok(())
    }

    async fn remove_new_ancestor_ids(&mut self) -> Result<(), NodecosmosError> {
        let mut node_and_descendant_ids = vec![self.reorder_data.node.id];
        node_and_descendant_ids.extend(self.reorder_data.descendant_ids.clone());
        let mut values = vec![];

        for id in node_and_descendant_ids {
            let branch_id = self.reorder_data.branch_id;
            values.push((&self.reorder_data.added_ancestor_ids, id, branch_id));
        }

        Node::unlogged_statement_batch()
            .chunked_statements(
                &self.db_session,
                Node::PULL_ANCESTOR_IDS_QUERY,
                values,
                crate::constants::BATCH_CHUNK_SIZE,
            )
            .await
            .map_err(|err| {
                error!("remove_new_ancestor_ids: {:?}", err);
                return err;
            })?;

        Ok(())
    }

    async fn append_old_ancestor_ids(&mut self) -> Result<(), NodecosmosError> {
        let mut node_and_descendant_ids = vec![self.reorder_data.node.id];
        node_and_descendant_ids.extend(self.reorder_data.descendant_ids.clone());
        let mut values = vec![];

        for id in node_and_descendant_ids {
            values.push((&self.reorder_data.removed_ancestor_ids, id, self.reorder_data.branch_id));
        }

        Node::unlogged_statement_batch()
            .chunked_statements(
                &self.db_session,
                Node::PUSH_ANCESTOR_IDS_QUERY,
                values,
                crate::constants::BATCH_CHUNK_SIZE,
            )
            .await
            .map_err(|err| {
                error!("append_old_ancestor_ids: {:?}", err);
                return err;
            })?;

        Ok(())
    }

    fn serialize_and_store_to_disk(&mut self) {
        let serialized = serde_json::to_string(&self.reorder_data).expect("Error in serializing recovery data");
        let filename = format!("{}{}.json", RECOVER_FILE_PREFIX, self.reorder_data.tree_root.id);
        let path = format!("{}/{}", RECOVERY_DATA_DIR, filename);
        let path_buf = Path::new(&path);

        // We only preserve the first recovery data file
        // as structure might be compromised later on if
        // user try to reorder again.
        if path_buf.exists() {
            error!("Recovery data file already exists: {}", path);

            return;
        }

        let res = std::fs::write(path.clone(), serialized);
        match res {
            Ok(_) => warn!("Recovery data saved to file: {}", path),
            Err(err) => error!("Error in saving recovery data: {}", err),
        }
    }
}
