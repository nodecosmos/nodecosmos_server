use crate::api::data::RequestData;
use crate::api::types::ActionTypes;
use crate::errors::NodecosmosError;
use crate::models::branch::{Branch, BranchStatus};
use crate::models::description::{find_description, Description};
use crate::models::node::context::Context;
use crate::models::node::reorder::ReorderParams;
use crate::models::node::{find_update_title_node, Node, UpdateTitleNode};
use crate::models::traits::cloned_ref::ClonedRef;
use crate::models::traits::{Branchable, GroupById, GroupByObjId};
use crate::models::udts::TextChange;
use crate::models::utils::file::read_file_names;
use charybdis::operations::{DeleteWithCallbacks, Find, InsertWithCallbacks, Update, UpdateWithCallbacks};
use charybdis::options::Consistency;
use charybdis::types::Uuid;
use log::{error, warn};
use scylla::CachingSession;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::create_dir_all;

const RECOVERY_DATA_DIR: &str = "tmp/merge-recovery";
const RECOVER_FILE_PREFIX: &str = "merge_recovery_data";

pub struct MergeError {
    pub inner: NodecosmosError,
    pub branch: Branch,
}

#[derive(Clone, Copy, Serialize, Deserialize, PartialOrd, PartialEq, Debug)]
pub enum MergeStep {
    Start = 0,
    RestoreNodes = 1,
    CreateNodes = 2,
    DeleteNodes = 3,
    ReorderNodes = 4,
    UpdateNodesTitles = 5,
    UpdateNodesDescription = 6,
    Finish = 7,
}

impl MergeStep {
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
            0 => MergeStep::Start,
            1 => MergeStep::RestoreNodes,
            2 => MergeStep::CreateNodes,
            3 => MergeStep::DeleteNodes,
            4 => MergeStep::ReorderNodes,
            5 => MergeStep::UpdateNodesTitles,
            6 => MergeStep::UpdateNodesDescription,
            7 => MergeStep::Finish,
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
    original_title_nodes: Option<HashMap<Uuid, UpdateTitleNode>>,
    original_nodes_descriptions: Option<HashMap<Uuid, Description>>,
}

impl BranchMerge {
    async fn fetch_original_title_nodes(
        db_session: &CachingSession,
        branch: &Branch,
    ) -> Result<Option<HashMap<Uuid, UpdateTitleNode>>, NodecosmosError> {
        if let Some(ids) = &branch.edited_node_titles {
            let nodes_by_id = find_update_title_node!("id IN ? AND branch_id IN ?", (ids, ids))
                .execute(db_session)
                .await?
                .group_by_id()
                .await?;

            return Ok(Some(nodes_by_id));
        }

        Ok(None)
    }

    async fn fetch_original_nodes_description(
        db_session: &CachingSession,
        branch: &Branch,
    ) -> Result<Option<HashMap<Uuid, Description>>, NodecosmosError> {
        if let Some(ids) = &branch.edited_nodes_descriptions {
            let nodes_by_id = find_description!("object_id IN ? AND branch_id IN ?", (ids, ids))
                .execute(db_session)
                .await?
                .group_by_obj_id()
                .await?;

            return Ok(Some(nodes_by_id));
        }

        Ok(None)
    }

    pub async fn run(branch: Branch, data: &RequestData) -> Result<Self, MergeError> {
        let original_title_nodes = match Self::fetch_original_title_nodes(data.db_session(), &branch).await {
            Ok(nodes) => nodes,
            Err(e) => return Err(MergeError { inner: e, branch }), // Early return on error
        };

        let original_nodes_descriptions = match Self::fetch_original_nodes_description(data.db_session(), &branch).await
        {
            Ok(nodes) => nodes,
            Err(e) => return Err(MergeError { inner: e, branch }), // Early return on error, no issue with moved value
        };

        let mut branch_merge = BranchMerge {
            branch,
            merge_step: MergeStep::Start,
            original_title_nodes,
            original_nodes_descriptions,
        };

        match branch_merge.merge(data).await {
            Ok(_) => {
                branch_merge.branch.status = Some(BranchStatus::Merged.to_string());
                Ok(branch_merge)
            }
            Err(e) => match branch_merge.recover(data).await {
                Ok(_) => {
                    branch_merge.branch.status = Some(BranchStatus::Recovered.to_string());
                    Err(MergeError {
                        inner: e,
                        branch: branch_merge.branch,
                    })
                }
                Err(recovery_err) => {
                    branch_merge.branch.status = Some(BranchStatus::RecoveryFailed.to_string());

                    error!("Mere::Failed to recover: {}", recovery_err);
                    branch_merge.serialize_and_store_to_disk();

                    Err(MergeError {
                        inner: NodecosmosError::FatalMergeError(format!("Failed to merge and recover: {}", e)),
                        branch: branch_merge.branch,
                    })
                }
            },
        }
    }

    pub async fn recover_from_stored_data(data: &RequestData) {
        create_dir_all(RECOVERY_DATA_DIR).unwrap();
        let files = read_file_names(RECOVERY_DATA_DIR, RECOVER_FILE_PREFIX).await;

        for file in files {
            let serialized = std::fs::read_to_string(file.clone()).unwrap();
            let mut branch_merge: BranchMerge = serde_json::from_str(&serialized)
                .map_err(|err| {
                    error!(
                        "Error in deserializing recovery data from file {}: {}",
                        file.clone(),
                        err
                    );
                })
                .unwrap();
            std::fs::remove_file(file.clone()).unwrap();

            if let Err(err) = branch_merge.recover(data).await {
                error!("Error in recovery from file {}: {}", file, err);
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
        while self.merge_step < MergeStep::Finish {
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
        while self.merge_step > MergeStep::Start {
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
        let node = self.branch.node(data.db_session()).await?;

        data.resource_locker()
            .unlock_resource(node.root_id, node.branchise_id(node.root_id))
            .await?;
        data.resource_locker()
            .unlock_resource_action(ActionTypes::Merge, node.root_id, node.branchise_id(node.root_id))
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
            Ok(_) => warn!("Merge Recovery data saved to file: {}", path),
            Err(err) => error!("Error in saving recovery data: {}", err),
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
                merge_node.ctx = Context::Merge;
                merge_node.branch_id = merge_node.id;
                merge_node.owner_id = node.owner_id;
                merge_node.editor_ids = node.editor_ids.clone();
                merge_node
                    .insert_cb(data)
                    .consistency(Consistency::All)
                    .execute(data.db_session())
                    .await?;
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
                merge_node.delete_cb(data).execute(data.db_session()).await?;
            }
        }

        Ok(())
    }

    async fn delete_nodes(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        let deleted_node_ids = self.branch.deleted_nodes.cloned_ref();

        for deleted_node_id in deleted_node_ids.clone() {
            let deleted_node = Node::find_by_id_and_branch_id(deleted_node_id, deleted_node_id)
                .execute(data.db_session())
                .await
                .ok();

            match deleted_node {
                Some(mut deleted_node) => {
                    if let Some(ancestor_ids) = &deleted_node.ancestor_ids {
                        if ancestor_ids.iter().any(|id| deleted_node_ids.contains(id)) {
                            // skip deletion of node if it has an ancestor that is also deleted as
                            // it will be removed in the callback
                            continue;
                        }

                        deleted_node.delete_cb(data).execute(data.db_session()).await?;
                    }
                }
                None => {
                    warn!(
                        "Failed to find deleted node with id {} and branch id {}",
                        deleted_node_id, deleted_node_id
                    );
                }
            }
        }

        Ok(())
    }

    async fn undo_delete_nodes(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        let deleted_node_ids = self.branch.deleted_nodes.cloned_ref();

        for deleted_node_id in deleted_node_ids.clone() {
            let deleted_node = Node::find_by_id_and_branch_id(deleted_node_id, deleted_node_id)
                .execute(data.db_session())
                .await
                .ok();

            match deleted_node {
                Some(mut deleted_node) => {
                    if let Some(ancestor_ids) = &deleted_node.ancestor_ids {
                        if ancestor_ids.iter().any(|id| deleted_node_ids.contains(id)) {
                            continue;
                        }

                        deleted_node.insert_cb(data).execute(data.db_session()).await?;
                    }
                }
                None => {
                    warn!(
                        "Failed to find deleted node with id {} and branch id {}",
                        deleted_node_id, deleted_node_id
                    );
                }
            }
        }

        Ok(())
    }

    async fn reorder_nodes(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        if let Some(reordered_nodes_data) = &self.branch.reordered_nodes_data() {
            for reorder_node_data in reordered_nodes_data {
                let node = Node::find_by_primary_key_value(&(reorder_node_data.id, reorder_node_data.id))
                    .execute(data.db_session())
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
                            error!("Failed to process with reorder: {:?}", e);
                        }
                    }
                    Err(e) => {
                        error!(
                            "Failed to find node with id {} and branch id {}: {:?}",
                            reorder_node_data.id, reorder_node_data.id, e
                        )
                    }
                }
            }
        }
        Ok(())
    }

    async fn undo_reorder_nodes(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        if let Some(reordered_nodes_data) = &self.branch.reordered_nodes_data() {
            for reorder_node_data in reordered_nodes_data {
                let node = Node::find_by_primary_key_value(&(reorder_node_data.id, reorder_node_data.id))
                    .execute(data.db_session())
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
                            error!("Failed to process with reorder: {:?}", e);
                        }
                    }
                    Err(e) => {
                        error!(
                            "Failed to find node with id {} and branch id {}: {:?}",
                            reorder_node_data.id, reorder_node_data.id, e
                        )
                    }
                }
            }
        }

        Ok(())
    }

    async fn update_nodes_titles(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        let original_title_nodes = match self.original_title_nodes.as_ref() {
            Some(nodes) => nodes,
            None => return Ok(()),
        };

        let edited_node_titles = match self.branch.edited_title_nodes(data.db_session()).await? {
            Some(titles) => titles,
            None => return Ok(()),
        };

        for mut edited_node_title in edited_node_titles {
            if let Some(original_node) = original_title_nodes.get(&edited_node_title.id) {
                if original_node.title != edited_node_title.title {
                    let mut text_change = TextChange::new();
                    text_change.assign_old(Some(original_node.title.clone()));
                    text_change.assign_new(Some(edited_node_title.title.clone()));

                    edited_node_title.branch_id = edited_node_title.id;
                    edited_node_title.update_cb(data).execute(data.db_session()).await?;

                    self.branch
                        .push_title_change_by_object(edited_node_title.id, text_change);
                }
            }
        }

        Ok(())
    }

    async fn undo_update_nodes_titles(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        if let Some(original_title_nodes) = &mut self.original_title_nodes {
            for original_title_node in original_title_nodes.values_mut() {
                original_title_node.update_cb(data).execute(data.db_session()).await?;
            }
        }

        Ok(())
    }

    async fn update_nodes_description(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        let edited_node_descriptions = match self.branch.edited_nodes_descriptions(data.db_session()).await? {
            Some(descriptions) => descriptions.clone(),
            None => return Ok(()),
        };

        let mut default = HashMap::default();
        let original_nodes_description = self
            .original_nodes_descriptions
            .as_mut()
            .unwrap_or_else(|| &mut default);

        for edited_node_description in edited_node_descriptions {
            let object_id = edited_node_description.object_id;
            let mut default_description = Description {
                object_id,
                branch_id: object_id,
                ..Default::default()
            };

            let original = original_nodes_description
                .get_mut(&object_id)
                .unwrap_or_else(|| &mut default_description);

            // init text change for remembrance of diff between old and new description
            let mut text_change = TextChange::new();
            text_change.assign_old(original.markdown.clone());

            // description merge is handled within before_insert callback
            original.base64 = edited_node_description.base64.clone();
            original.insert_cb(data).execute(data.db_session()).await?;

            // update text change with new description
            text_change.assign_new(original.markdown.clone());
            self.branch.push_description_change_by_object(object_id, text_change);
        }

        Ok(())
    }

    async fn undo_update_nodes_description(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        if let Some(original_nodes_description) = &mut self.original_nodes_descriptions {
            for original_node_description in original_nodes_description.values_mut() {
                // description merge is handled within before_insert callback, so we use update to revert as
                // it will not trigger merge logic
                original_node_description
                    .update_cb(data)
                    .execute(data.db_session())
                    .await?;
            }
        }

        Ok(())
    }
}

impl Branch {
    pub async fn merge(mut self, data: &RequestData) -> Result<Self, MergeError> {
        if let Err(e) = self.validate_no_existing_conflicts().await {
            return Err(MergeError { inner: e, branch: self });
        }

        if let Err(e) = self.check_conflicts(data.db_session()).await {
            return Err(MergeError { inner: e, branch: self });
        }

        let merge = BranchMerge::run(self, data).await?;

        let res = merge.branch.update().execute(data.db_session()).await;

        if let Err(e) = res {
            return Err(MergeError {
                inner: NodecosmosError::from(e),
                branch: merge.branch,
            });
        }

        Ok(merge.branch)
    }
}
