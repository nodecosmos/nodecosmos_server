use crate::api::data::RequestData;
use crate::api::types::{ActionObject, ActionTypes};
use crate::errors::NodecosmosError;
use crate::models::branch::Branch;
use crate::models::node::reorder::ReorderParams;
use crate::models::node::{
    find_update_description_node, find_update_title_node, Node, UpdateDescriptionNode, UpdateTitleNode,
};
use crate::models::udts::{Conflict, ConflictStatus};
use crate::utils::cloned_ref::ClonedRef;
use crate::utils::file::read_file_names;
use crate::utils::logger::{log_error, log_fatal, log_warning};
use charybdis::operations::{DeleteWithExtCallbacks, Find, InsertWithExtCallbacks, UpdateWithExtCallbacks};
use charybdis::types::Uuid;
use scylla::CachingSession;
use serde::{Deserialize, Serialize};
use std::fs::create_dir_all;
use std::future::Future;

const RECOVERY_DATA_DIR: &str = "tmp/merge-recovery";
const RECOVER_FILE_PREFIX: &str = "merge_recovery_data";

#[derive(Clone, Copy, Serialize, Deserialize, PartialOrd, PartialEq)]
pub enum MergeStep {
    None = 0,
    RestoreNodes = 1,
    CreateNodes = 2,
    DeleteNodes = 3,
    ReorderNodes = 4,
    UpdateNodesTitles = 5,
    UpdateNodesDescription = 6,
}

impl MergeStep {
    const LAST_STEP: MergeStep = MergeStep::UpdateNodesDescription;

    pub fn increment(&mut self) {
        *self = MergeStep::from(*self as u8 + 1);
    }

    pub fn decrement(&mut self) {
        *self = MergeStep::from(*self as u8 - 1);
    }
}

impl From<u8> for MergeStep {
    fn from(value: u8) -> Self {
        match value {
            0 => MergeStep::None,
            1 => MergeStep::RestoreNodes,
            2 => MergeStep::CreateNodes,
            3 => MergeStep::DeleteNodes,
            4 => MergeStep::ReorderNodes,
            5 => MergeStep::UpdateNodesTitles,
            6 => MergeStep::UpdateNodesDescription,
            _ => panic!("Invalid MergeStep value: {}", value),
        }
    }
}

/// ScyllaDB does not support transactions like SQL databases. We need to introduce a pattern to handle merge failures.
/// We use the SAGA pattern to handle merge failures.
/// We store the state of the merge process in a file and recover from it.
#[derive(Serialize, Deserialize)]
pub struct BranchMerge {
    branch: Branch,
    merge_step: MergeStep,
    original_title_nodes: Option<Vec<UpdateTitleNode>>,
    original_description_nodes: Option<Vec<UpdateDescriptionNode>>,
}

impl BranchMerge {
    async fn fetch_original_title_nodes(
        session: &CachingSession,
        branch: &Branch,
    ) -> Result<Option<Vec<UpdateTitleNode>>, NodecosmosError> {
        if let Some(ids) = &branch.edited_node_titles {
            let res = find_update_title_node!(session, "branch_id IN ? AND id IN ?", (ids, ids))
                .await?
                .try_collect()
                .await?;

            return Ok(Some(res));
        }

        Ok(None)
    }

    async fn fetch_original_description_nodes(
        session: &CachingSession,
        branch: &Branch,
    ) -> Result<Option<Vec<UpdateDescriptionNode>>, NodecosmosError> {
        if let Some(ids) = &branch.edited_node_descriptions {
            let res = find_update_description_node!(session, "branch_id IN ? AND id IN ?", (ids, ids))
                .await?
                .try_collect()
                .await?;

            return Ok(Some(res));
        }

        Ok(None)
    }

    pub async fn run(branch: Branch, data: &RequestData) -> Result<(), NodecosmosError> {
        let session = data.db_session();

        // Simplified fetching of nodes
        let original_title_nodes = Self::fetch_original_title_nodes(&session, &branch).await?;
        let original_description_nodes = Self::fetch_original_description_nodes(&session, &branch).await?;

        let mut branch_merge = BranchMerge {
            branch,
            merge_step: MergeStep::None,
            original_title_nodes,
            original_description_nodes,
        };

        // Use early return for error handling
        if let Err(e) = branch_merge.merge(data).await {
            if let Err(recovery_err) = branch_merge.recover(data).await {
                log_fatal(format!("Failed to recover: {}", recovery_err));
                branch_merge.serialize_and_store_to_disk();
                return Err(NodecosmosError::FatalMergeError(format!(
                    "Failed to merge and recover: {}",
                    e
                )));
            }

            branch_merge.unlock_resource(data).await?;
            return Err(NodecosmosError::MergeError(format!("Failed to merge: {}", e)));
        }

        Ok(())
    }

    pub async fn recover_from_stored_data(data: &RequestData) {
        create_dir_all(RECOVERY_DATA_DIR).unwrap();
        let files = read_file_names(RECOVERY_DATA_DIR, RECOVER_FILE_PREFIX).await;

        for file in files {
            let serialized = std::fs::read_to_string(file.clone()).unwrap();
            let mut branch_merge: BranchMerge = serde_json::from_str(&serialized)
                .map_err(|err| {
                    log_fatal(format!(
                        "Error in deserializing recovery data from file {}: {}",
                        file.clone(),
                        err
                    ));
                })
                .unwrap();
            std::fs::remove_file(file.clone()).unwrap();

            if let Err(err) = branch_merge.recover(data).await {
                log_fatal(format!("Error in recovery from file {}: {}", file, err));
                branch_merge.serialize_and_store_to_disk();
                continue;
            }

            branch_merge.unlock_resource(data).await.expect(
                format!(
                    "Merge Recovery: Failed to unlock resource: branch_id: {}",
                    branch_merge.branch.id
                )
                .as_str(),
            );
        }
    }

    /// Merge the branch
    async fn merge(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        self.branch.validate_no_existing_conflicts().await?;
        self.branch.check_conflicts(data.db_session()).await?;

        while self.merge_step != MergeStep::LAST_STEP {
            match self.merge_step {
                MergeStep::RestoreNodes => self.restore_nodes(data).await?,
                MergeStep::CreateNodes => self.create_nodes(data).await?,
                MergeStep::DeleteNodes => self.delete_nodes(data).await?,
                MergeStep::ReorderNodes => self.reorder_nodes(data).await?,
                MergeStep::UpdateNodesTitles => self.update_nodes_titles(data).await?,
                MergeStep::UpdateNodesDescription => self.update_nodes_description(data).await?,
                _ => {}
            }

            self.merge_step.increment();
        }

        Ok(())
    }

    /// Recover from merge failure in reverse order
    pub async fn recover(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        while self.merge_step != MergeStep::None {
            match self.merge_step {
                MergeStep::RestoreNodes => self.undo_restore_nodes(data).await?,
                MergeStep::CreateNodes => self.undo_create_nodes(data).await?,
                MergeStep::DeleteNodes => self.undo_delete_nodes(data).await?,
                MergeStep::ReorderNodes => self.undo_reorder_nodes(data).await?,
                MergeStep::UpdateNodesTitles => self.undo_update_nodes_titles(data).await?,
                MergeStep::UpdateNodesDescription => self.undo_update_nodes_description(data).await?,
                _ => {}
            }

            self.merge_step.decrement();
        }

        Ok(())
    }

    async fn unlock_resource(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        let root_id = self.branch.node(data.db_session()).await?.root_id;

        data.resource_locker().unlock_resource(&root_id.to_string()).await?;
        data.resource_locker()
            .unlock_resource_action(ActionTypes::Merge, &root_id.to_string())
            .await?;

        Ok(())
    }

    fn serialize_and_store_to_disk(&self) {
        // serialize branch_merge and store to disk
        let serialized = serde_json::to_string(self).unwrap();
        let filename = format!("{}{}.json", RECOVER_FILE_PREFIX, self.branch.id);
        let path = format!("{}/{}", RECOVERY_DATA_DIR, filename);
        let res = std::fs::write(path.clone(), serialized);

        match res {
            Ok(_) => log_warning(format!("Merge Recovery data saved to file: {}", path)),
            Err(err) => log_fatal(format!("Error in saving recovery data: {}", err)),
        }
    }

    async fn restore_nodes(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        let restored_nodes = self.branch.restored_nodes(data.db_session()).await?;

        self.insert_nodes(data, restored_nodes).await?;

        Ok(())
    }

    async fn undo_restore_nodes(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        let restored_nodes = self.branch.restored_nodes(data.db_session()).await?;

        self.delete_inserted_nodes(data, restored_nodes).await?;

        Ok(())
    }

    async fn create_nodes(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        let created_nodes = self.branch.created_nodes(data.db_session()).await?;

        self.insert_nodes(data, created_nodes).await?;

        Ok(())
    }

    async fn undo_create_nodes(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        let created_nodes = self.branch.created_nodes(data.db_session()).await?;

        self.delete_inserted_nodes(data, created_nodes).await?;

        Ok(())
    }

    async fn insert_nodes(
        &mut self,
        data: &RequestData,
        merge_nodes: Option<Vec<Node>>,
    ) -> Result<(), NodecosmosError> {
        let node = self.branch.node(data.db_session()).await?;

        if let Some(merge_nodes) = merge_nodes {
            for mut merge_node in merge_nodes {
                merge_node.merge_ctx = true;
                merge_node.branch_id = merge_node.id;
                merge_node.owner_id = node.owner_id;
                merge_node.owner_type = node.owner_type.clone();
                merge_node.editor_ids = node.editor_ids.clone();
                merge_node.insert_cb(data.db_session(), data).await?;
            }
        }

        Ok(())
    }

    async fn delete_inserted_nodes(
        &mut self,
        data: &RequestData,
        merge_nodes: Option<Vec<Node>>,
    ) -> Result<(), NodecosmosError> {
        if let Some(merge_nodes) = merge_nodes {
            for mut merge_node in merge_nodes {
                merge_node.branch_id = merge_node.id;
                merge_node.delete_cb(data.db_session(), data).await?;
            }
        }

        Ok(())
    }

    async fn delete_nodes(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        let deleted_node_ids = self.branch.deleted_nodes.cloned_ref();

        for deleted_node_id in deleted_node_ids.clone() {
            let deleted_node = Node::find_by_id_and_branch_id(data.db_session(), deleted_node_id, deleted_node_id)
                .await
                .ok();

            match deleted_node {
                Some(mut deleted_node) => {
                    if let Some(ancestor_ids) = deleted_node.ancestor_ids.clone() {
                        if ancestor_ids.iter().any(|id| deleted_node_ids.contains(id)) {
                            // skip deletion of node if it has an ancestor that is also deleted as it will be removed in the callback
                            continue;
                        }

                        deleted_node.delete_cb(data.db_session(), data).await?;
                    }
                }
                None => {
                    log_warning(format!(
                        "Failed to find deleted node with id {} and branch id {}",
                        deleted_node_id, deleted_node_id
                    ));
                }
            }
        }

        Ok(())
    }

    async fn undo_delete_nodes(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        let deleted_node_ids = self.branch.deleted_nodes.cloned_ref();

        for deleted_node_id in deleted_node_ids.clone() {
            let deleted_node = Node::find_by_id_and_branch_id(data.db_session(), deleted_node_id, deleted_node_id)
                .await
                .ok();

            match deleted_node {
                Some(mut deleted_node) => {
                    if let Some(ancestor_ids) = deleted_node.ancestor_ids.clone() {
                        if ancestor_ids.iter().any(|id| deleted_node_ids.contains(id)) {
                            continue;
                        }

                        deleted_node.insert_cb(data.db_session(), data).await?;
                    }
                }
                None => {
                    log_warning(format!(
                        "Failed to find deleted node with id {} and branch id {}",
                        deleted_node_id, deleted_node_id
                    ));
                }
            }
        }

        Ok(())
    }

    async fn reorder_nodes(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        let deleted_node_ids = self.branch.deleted_nodes.cloned_ref();
        let created_node_ids = self.branch.created_nodes.cloned_ref();

        if let Some(reordered_nodes) = &self.branch.reordered_nodes {
            for reorder_node_data in reordered_nodes {
                if deleted_node_ids.contains(&reorder_node_data.id) || created_node_ids.contains(&reorder_node_data.id)
                {
                    continue;
                };

                let node =
                    Node::find_by_primary_key_value(data.db_session(), (reorder_node_data.id, reorder_node_data.id))
                        .await;

                match node {
                    Ok(node) => {
                        let reorder_params = ReorderParams {
                            id: reorder_node_data.id,
                            branch_id: reorder_node_data.id,
                            new_parent_id: reorder_node_data.new_parent_id,
                            new_upper_sibling_id: reorder_node_data.new_upper_sibling_id,
                            new_lower_sibling_id: reorder_node_data.new_lower_sibling_id,
                            new_order_index: None,
                        };
                        let res = node.reorder(data, reorder_params).await;

                        if let Err(e) = res {
                            log_error(format!("Failed to process with reorder: {:?}", e));
                        }
                    }
                    Err(e) => {
                        log_error(format!(
                            "Failed to find node with id {} and branch id {}: {:?}",
                            reorder_node_data.id, reorder_node_data.id, e
                        ));
                    }
                }
            }
        }
        Ok(())
    }

    async fn undo_reorder_nodes(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        let deleted_node_ids = self.branch.deleted_nodes.cloned_ref();
        let created_node_ids = self.branch.created_nodes.cloned_ref();

        if let Some(reordered_nodes) = &self.branch.reordered_nodes {
            for reorder_node_data in reordered_nodes {
                if deleted_node_ids.contains(&reorder_node_data.id) || created_node_ids.contains(&reorder_node_data.id)
                {
                    continue;
                };

                let node =
                    Node::find_by_primary_key_value(data.db_session(), (reorder_node_data.id, reorder_node_data.id))
                        .await;

                match node {
                    Ok(node) => {
                        let reorder_params = ReorderParams {
                            id: reorder_node_data.id,
                            branch_id: reorder_node_data.id,
                            new_parent_id: reorder_node_data.old_parent_id,
                            new_order_index: Some(reorder_node_data.old_order_index),
                            new_upper_sibling_id: None,
                            new_lower_sibling_id: None,
                        };
                        let res = node.reorder(data, reorder_params).await;

                        if let Err(e) = res {
                            log_error(format!("Failed to process with reorder: {:?}", e));
                        }
                    }
                    Err(e) => {
                        log_error(format!(
                            "Failed to find node with id {} and branch id {}: {:?}",
                            reorder_node_data.id, reorder_node_data.id, e
                        ));
                    }
                }
            }
        }

        Ok(())
    }

    async fn update_nodes_titles(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        if let Some(edited_node_titles) = self.branch.edited_title_nodes(data.db_session()).await? {
            for mut edited_node_title in edited_node_titles {
                edited_node_title.branch_id = edited_node_title.id;
                edited_node_title.update_cb(data.db_session(), data).await?;
            }
        }

        Ok(())
    }

    async fn undo_update_nodes_titles(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        if let Some(original_title_nodes) = self.original_title_nodes.clone() {
            for mut original_title_node in original_title_nodes {
                original_title_node.update_cb(data.db_session(), data).await?;
            }
        }

        Ok(())
    }

    async fn update_nodes_description(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        let created_node_ids = &self.branch.created_nodes.cloned_ref();
        let deleted_node_ids = self.branch.deleted_nodes.cloned_ref();

        if let Some(edited_description_nodes) = self.branch.edited_description_nodes(data.db_session()).await? {
            for edited_description_node in edited_description_nodes {
                if created_node_ids.contains(&edited_description_node.id)
                    || deleted_node_ids.contains(&edited_description_node.id)
                {
                    continue;
                };

                let mut original = UpdateDescriptionNode::find_by_primary_key_value(
                    data.db_session(),
                    (edited_description_node.id, edited_description_node.id),
                )
                .await
                .map_err(|e| {
                    log_error(format!(
                        "Failed to find update description node with id {}: {:?}",
                        edited_description_node.id, e
                    ));
                    e
                })?;

                original.description_base64 = edited_description_node.description_base64;

                // description merge is handled within before_update callback
                original.update_cb(data.db_session(), data).await?;
            }
        }

        Ok(())
    }

    async fn undo_update_nodes_description(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        if let Some(original_description_nodes) = self.original_description_nodes.clone() {
            for mut original_description_node in original_description_nodes {
                original_description_node.recovery_ctx = true;

                original_description_node.update_cb(data.db_session(), data).await?;
            }
        }

        Ok(())
    }
}

/// introduce SAGA pattern for merge failure handling
impl Branch {
    pub async fn merge(&self, data: &RequestData) -> Result<(), NodecosmosError> {
        BranchMerge::run(self.clone(), data).await
    }
}
