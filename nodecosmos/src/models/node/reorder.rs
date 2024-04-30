use charybdis::batch::ModelBatch;
use charybdis::operations::Update;
use charybdis::types::{Double, Uuid};
use log::{error, warn};
use scylla::CachingSession;
use serde::{Deserialize, Serialize};
use std::fs::create_dir_all;

use nodecosmos_macros::Branchable;

use crate::api::data::RequestData;
use crate::api::types::ActionTypes;
use crate::errors::NodecosmosError;
use crate::models::branch::update::BranchUpdate;
use crate::models::branch::Branch;
use crate::models::node::reorder::data::ReorderData;
use crate::models::node::reorder::validator::ReorderValidator;
use crate::models::node::{Node, UpdateOrderNode};
use crate::models::node_descendant::NodeDescendant;
use crate::models::traits::Branchable;
use crate::models::udts::BranchReorderData;
use crate::models::utils::file::read_file_names;

pub mod data;
mod validator;

const RECOVERY_DATA_DIR: &str = "tmp/reorder-recovery";
const RECOVER_FILE_PREFIX: &str = "reorder_recover_data";

#[derive(Clone, Copy, Serialize, Deserialize, PartialOrd, PartialEq, Debug)]
pub enum ReorderStep {
    Start = 0,
    UpdateNodeOrderIndex = 1,
    RemoveNodeFromOldAncestors = 2,
    AddNodeToNewAncestors = 3,
    PullRemovedAncestorsFromNode = 4,
    PullRemovedAncestorsFromDescendants = 5,
    DeleteNodeDescendantsFromRemovedAncestors = 6,
    PushAddedAncestorsToNode = 7,
    PushAddedAncestorsToDescendants = 8,
    InsertNodeDescendantsToAddedAncestors = 9,
    UpdateBranch = 10,
    Finish = 11,
}

impl ReorderStep {
    pub fn increment(&mut self) {
        *self = ReorderStep::from(*self as u8 + 1);
    }

    pub fn decrement(&mut self) {
        *self = ReorderStep::from(*self as u8 - 1);
    }
}

impl From<u8> for ReorderStep {
    fn from(value: u8) -> Self {
        match value {
            0 => ReorderStep::Start,
            1 => ReorderStep::UpdateNodeOrderIndex,
            2 => ReorderStep::RemoveNodeFromOldAncestors,
            3 => ReorderStep::AddNodeToNewAncestors,
            4 => ReorderStep::PullRemovedAncestorsFromNode,
            5 => ReorderStep::PullRemovedAncestorsFromDescendants,
            6 => ReorderStep::DeleteNodeDescendantsFromRemovedAncestors,
            7 => ReorderStep::PushAddedAncestorsToNode,
            8 => ReorderStep::PushAddedAncestorsToDescendants,
            9 => ReorderStep::InsertNodeDescendantsToAddedAncestors,
            10 => ReorderStep::UpdateBranch,
            11 => ReorderStep::Finish,
            _ => panic!("Invalid value for ReorderStep"),
        }
    }
}

#[derive(Serialize, Deserialize, Branchable)]
#[serde(rename_all = "camelCase")]
pub struct ReorderParams {
    #[branch(original_id)]
    pub root_id: Uuid,
    pub branch_id: Uuid,
    pub id: Uuid,
    pub new_parent_id: Uuid,
    pub new_upper_sibling_id: Option<Uuid>,
    pub new_lower_sibling_id: Option<Uuid>,
    // we provide order_index only in context of merge recovery
    pub new_order_index: Option<Double>,
}

#[derive(Serialize, Deserialize)]
pub struct Reorder {
    pub reorder_data: ReorderData,
    pub reorder_step: ReorderStep,
}

impl Reorder {
    pub async fn recover_from_stored_data(data: &RequestData) {
        create_dir_all(RECOVERY_DATA_DIR).unwrap();
        let files = read_file_names(RECOVERY_DATA_DIR, RECOVER_FILE_PREFIX).await;

        for file in files {
            let serialized = std::fs::read_to_string(file.clone()).unwrap();
            let mut reorder: Reorder = serde_json::from_str(&serialized)
                .map_err(|err| {
                    error!(
                        "Error in deserializing recovery data from file {}: {}",
                        file.clone(),
                        err
                    );
                })
                .unwrap();
            std::fs::remove_file(file.clone()).unwrap();

            if let Err(err) = reorder.recover(data.db_session()).await {
                error!("Error in recovery from file {}: {}", file, err);
                reorder.serialize_and_store_to_disk();
                continue;
            }

            reorder.unlock_resource(data).await.expect(
                format!(
                    "Reorder Recovery: Failed to unlock resource: node_id: {}",
                    reorder.reorder_data.node.id
                )
                .as_str(),
            );
        }
    }

    pub fn new(data: ReorderData) -> Reorder {
        Self {
            reorder_data: data,
            reorder_step: ReorderStep::Start,
        }
    }

    async fn unlock_resource(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        data.resource_locker()
            .unlock_resource(self.reorder_data.node.root_id, self.reorder_data.node.branch_id)
            .await?;
        data.resource_locker()
            .unlock_resource_action(
                ActionTypes::Merge,
                self.reorder_data.node.root_id,
                self.reorder_data.node.branch_id,
            )
            .await?;

        Ok(())
    }

    pub async fn run(&mut self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        let res = self.execute_reorder(db_session).await;

        if let Err(err) = res {
            error!(
                "Reorder failed for node: {}\n! ERROR: {:?}",
                self.reorder_data.node.id, err
            );

            self.recover(db_session).await.map_err(|recover_err| {
                error!(
                    "Fatal Reorder recovery failed for node: {}\n! ERROR: {:?}",
                    self.reorder_data.node.id, recover_err
                );
                self.serialize_and_store_to_disk();

                NodecosmosError::FatalReorderError(err.to_string())
            })?;

            return Err(err);
        }

        Ok(())
    }

    async fn execute_reorder(&mut self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        while self.reorder_step < ReorderStep::Finish {
            match self.reorder_step {
                ReorderStep::Start => (),
                ReorderStep::UpdateNodeOrderIndex => self.update_node_order(db_session).await?,
                ReorderStep::RemoveNodeFromOldAncestors => self.remove_node_from_old_ancestors(db_session).await?,
                ReorderStep::AddNodeToNewAncestors => self.add_node_to_new_ancestors(db_session).await?,
                ReorderStep::PullRemovedAncestorsFromNode => self.pull_removed_ancestors_from_node(db_session).await?,
                ReorderStep::PullRemovedAncestorsFromDescendants => {
                    self.pull_removed_ancestors_from_descendants(db_session).await?
                }
                ReorderStep::DeleteNodeDescendantsFromRemovedAncestors => {
                    self.delete_node_descendants_from_removed_ancestors(db_session).await?
                }
                ReorderStep::PushAddedAncestorsToNode => self.push_added_ancestors_to_node(db_session).await?,
                ReorderStep::PushAddedAncestorsToDescendants => {
                    self.push_added_ancestors_to_descendants(db_session).await?
                }
                ReorderStep::InsertNodeDescendantsToAddedAncestors => {
                    self.insert_node_descendants_to_added_ancestors(db_session).await?
                }
                ReorderStep::UpdateBranch => self.update_branch(db_session).await?,
                ReorderStep::Finish => (),
            }

            self.reorder_step.increment();
        }

        Ok(())
    }

    async fn recover(&mut self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        while self.reorder_step > ReorderStep::Start {
            match self.reorder_step {
                ReorderStep::Start => (),
                ReorderStep::UpdateNodeOrderIndex => self.undo_update_node_order(db_session).await?,
                ReorderStep::RemoveNodeFromOldAncestors => self.undo_remove_node_from_old_ancestors(db_session).await?,
                ReorderStep::AddNodeToNewAncestors => self.undo_add_node_to_new_ancestors(db_session).await?,
                ReorderStep::PullRemovedAncestorsFromNode => {
                    self.undo_pull_removed_ancestors_from_node(db_session).await?
                }
                ReorderStep::PullRemovedAncestorsFromDescendants => {
                    self.undo_pull_removed_ancestors_from_descendants(db_session).await?
                }
                ReorderStep::DeleteNodeDescendantsFromRemovedAncestors => {
                    self.undo_delete_node_descendants_from_removed_ancestors(db_session)
                        .await?
                }
                ReorderStep::PushAddedAncestorsToNode => self.undo_push_added_ancestors_to_node(db_session).await?,
                ReorderStep::PushAddedAncestorsToDescendants => {
                    self.undo_push_added_ancestors_to_descendants(db_session).await?
                }
                ReorderStep::InsertNodeDescendantsToAddedAncestors => {
                    self.undo_insert_node_descendants_to_added_ancestors(db_session).await?
                }
                ReorderStep::UpdateBranch => self.undo_update_branch(db_session).await?,
                ReorderStep::Finish => (),
            }

            self.reorder_step.decrement();
        }

        Ok(())
    }

    async fn update_node_order(&mut self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        let update_order_node = UpdateOrderNode {
            id: self.reorder_data.node.id,
            branch_id: self.reorder_data.branch_id,
            root_id: self.reorder_data.node.root_id,
            parent_id: Some(self.reorder_data.new_parent_id),
            order_index: self.reorder_data.new_order_index,
        };

        update_order_node.update().execute(db_session).await?;

        Ok(())
    }

    async fn undo_update_node_order(&mut self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        let update_order_node = UpdateOrderNode {
            id: self.reorder_data.node.id,
            branch_id: self.reorder_data.branch_id,
            root_id: self.reorder_data.node.root_id,
            parent_id: Some(self.reorder_data.old_parent_id),
            order_index: self.reorder_data.old_order_index,
        };

        update_order_node.update().execute(db_session).await?;

        Ok(())
    }

    async fn remove_node_from_old_ancestors(&mut self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        let mut descendants_to_delete = vec![];

        for ancestor_id in &self.reorder_data.old_ancestor_ids {
            let descendant = NodeDescendant {
                root_id: self.reorder_data.node.root_id,
                branch_id: self.reorder_data.branch_id,
                node_id: *ancestor_id,
                id: self.reorder_data.node.id,
                order_index: self.reorder_data.old_order_index,
                ..Default::default()
            };

            descendants_to_delete.push(descendant);
        }

        NodeDescendant::unlogged_delete_batch()
            .chunked_delete(db_session, &descendants_to_delete, crate::constants::BATCH_CHUNK_SIZE)
            .await
            .map_err(|err| {
                error!("remove_node_from_removed_ancestors: {:?}", err);
                return err;
            })?;

        Ok(())
    }

    async fn undo_remove_node_from_old_ancestors(
        &mut self,
        db_session: &CachingSession,
    ) -> Result<(), NodecosmosError> {
        let mut descendants_to_add = vec![];

        for ancestor_id in &self.reorder_data.old_ancestor_ids {
            let descendant = NodeDescendant {
                root_id: self.reorder_data.node.root_id,
                branch_id: self.reorder_data.branch_id,
                node_id: *ancestor_id,
                id: self.reorder_data.node.id,
                order_index: self.reorder_data.old_order_index,
                title: self.reorder_data.node.title.clone(),
                parent_id: self.reorder_data.old_parent_id,
            };

            descendants_to_add.push(descendant);
        }

        NodeDescendant::unlogged_batch()
            .chunked_insert(db_session, &descendants_to_add, crate::constants::BATCH_CHUNK_SIZE)
            .await
            .map_err(|err| {
                error!("undo_remove_node_from_old_ancestors: {:?}", err);
                return err;
            })?;

        Ok(())
    }

    async fn delete_node_descendants_from_removed_ancestors(
        &mut self,
        db_session: &CachingSession,
    ) -> Result<(), NodecosmosError> {
        if self.reorder_data.parent_changed() {
            let mut descendants_to_delete = vec![];

            for ancestor_id in &self.reorder_data.removed_ancestor_ids {
                for descendant in &self.reorder_data.descendants {
                    let descendant = NodeDescendant {
                        root_id: self.reorder_data.node.root_id,
                        branch_id: self.reorder_data.branch_id,
                        node_id: *ancestor_id,
                        order_index: descendant.order_index,
                        id: descendant.id,
                        parent_id: descendant.parent_id,
                        title: String::default(),
                    };

                    descendants_to_delete.push(descendant);
                }
            }

            NodeDescendant::unlogged_delete_batch()
                .chunked_delete(db_session, &descendants_to_delete, crate::constants::BATCH_CHUNK_SIZE)
                .await
                .map_err(|err| {
                    error!("delete_node_descendants_from_removed_ancestors: {:?}", err);
                    return err;
                })?;
        }

        Ok(())
    }

    async fn undo_delete_node_descendants_from_removed_ancestors(
        &mut self,
        db_session: &CachingSession,
    ) -> Result<(), NodecosmosError> {
        if self.reorder_data.parent_changed() {
            let mut descendants_to_add = vec![];

            for ancestor_id in &self.reorder_data.removed_ancestor_ids {
                for descendant in &self.reorder_data.descendants {
                    let descendant = NodeDescendant {
                        root_id: self.reorder_data.node.root_id,
                        branch_id: self.reorder_data.branch_id,
                        node_id: *ancestor_id,
                        order_index: descendant.order_index,
                        id: descendant.id,
                        parent_id: descendant.parent_id,
                        title: descendant.title.clone(),
                    };

                    descendants_to_add.push(descendant);
                }
            }

            NodeDescendant::unlogged_batch()
                .chunked_insert(db_session, &descendants_to_add, crate::constants::BATCH_CHUNK_SIZE)
                .await
                .map_err(|err| {
                    error!("undo_delete_node_descendants_from_removed_ancestors: {:?}", err);
                    return err;
                })?;
        }

        Ok(())
    }

    async fn add_node_to_new_ancestors(&mut self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        let mut descendants_to_add = vec![];

        for ancestor_id in &self.reorder_data.new_ancestor_ids {
            let descendant = NodeDescendant {
                root_id: self.reorder_data.node.root_id,
                branch_id: self.reorder_data.branch_id,
                node_id: *ancestor_id,
                order_index: self.reorder_data.new_order_index,
                id: self.reorder_data.node.id,
                title: self.reorder_data.node.title.clone(),
                parent_id: self.reorder_data.new_parent_id,
            };

            descendants_to_add.push(descendant);
        }

        NodeDescendant::unlogged_batch()
            .chunked_insert(db_session, &descendants_to_add, crate::constants::BATCH_CHUNK_SIZE)
            .await
            .map_err(|err| {
                error!("add_node_to_new_ancestors: {:?}", err);
                return err;
            })?;

        Ok(())
    }

    async fn undo_add_node_to_new_ancestors(&mut self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        let mut descendants_to_delete = vec![];

        for ancestor_id in &self.reorder_data.new_ancestor_ids {
            let descendant = NodeDescendant {
                root_id: self.reorder_data.node.root_id,
                branch_id: self.reorder_data.branch_id,
                node_id: *ancestor_id,
                order_index: self.reorder_data.new_order_index,
                id: self.reorder_data.node.id,
                title: self.reorder_data.node.title.clone(),
                parent_id: self.reorder_data.new_parent_id,
            };

            descendants_to_delete.push(descendant);
        }

        NodeDescendant::unlogged_delete_batch()
            .chunked_delete(db_session, &descendants_to_delete, crate::constants::BATCH_CHUNK_SIZE)
            .await
            .map_err(|err| {
                error!("undo_add_node_to_new_ancestors: {:?}", err);
                return err;
            })?;

        Ok(())
    }

    async fn insert_node_descendants_to_added_ancestors(
        &mut self,
        db_session: &CachingSession,
    ) -> Result<(), NodecosmosError> {
        if self.reorder_data.parent_changed() {
            let mut descendants = vec![];

            for ancestor_id in &self.reorder_data.added_ancestor_ids {
                for descendant in &self.reorder_data.descendants {
                    let descendant = NodeDescendant {
                        root_id: self.reorder_data.node.root_id,
                        branch_id: self.reorder_data.branch_id,
                        node_id: *ancestor_id,
                        id: descendant.id,
                        order_index: descendant.order_index,
                        title: descendant.title.clone(),
                        parent_id: descendant.parent_id,
                    };

                    descendants.push(descendant);
                }
            }

            NodeDescendant::unlogged_batch()
                .chunked_insert(db_session, &descendants, crate::constants::BATCH_CHUNK_SIZE)
                .await
                .map_err(|err| {
                    error!("insert_node_descendants_to_added_ancestors: {:?}", err);
                    return err;
                })?;
        }

        Ok(())
    }

    async fn undo_insert_node_descendants_to_added_ancestors(
        &mut self,
        db_session: &CachingSession,
    ) -> Result<(), NodecosmosError> {
        if self.reorder_data.parent_changed() {
            let mut descendants = vec![];

            for ancestor_id in &self.reorder_data.added_ancestor_ids {
                for descendant in &self.reorder_data.descendants {
                    let descendant = NodeDescendant {
                        root_id: self.reorder_data.node.root_id,
                        branch_id: self.reorder_data.branch_id,
                        node_id: *ancestor_id,
                        id: descendant.id,
                        order_index: descendant.order_index,
                        title: descendant.title.clone(),
                        parent_id: descendant.parent_id,
                    };

                    descendants.push(descendant);
                }
            }

            NodeDescendant::unlogged_delete_batch()
                .chunked_delete(db_session, &descendants, crate::constants::BATCH_CHUNK_SIZE)
                .await
                .map_err(|err| {
                    error!("undo_insert_node_descendants_to_added_ancestors: {:?}", err);
                    return err;
                })?;
        }

        Ok(())
    }

    async fn pull_removed_ancestors_from_node(&mut self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        if self.reorder_data.parent_changed() {
            self.reorder_data
                .node
                .pull_ancestor_ids(&self.reorder_data.removed_ancestor_ids)
                .execute(db_session)
                .await?;
        }

        Ok(())
    }

    async fn undo_pull_removed_ancestors_from_node(
        &mut self,
        db_session: &CachingSession,
    ) -> Result<(), NodecosmosError> {
        if self.reorder_data.parent_changed() {
            self.reorder_data
                .node
                .push_ancestor_ids(&self.reorder_data.removed_ancestor_ids)
                .execute(db_session)
                .await?;
        }

        Ok(())
    }

    async fn pull_removed_ancestors_from_descendants(
        &mut self,
        db_session: &CachingSession,
    ) -> Result<(), NodecosmosError> {
        if self.reorder_data.parent_changed() {
            let mut values = vec![];

            for descendant_id in &self.reorder_data.descendant_ids {
                let val = (
                    &self.reorder_data.removed_ancestor_ids,
                    self.reorder_data.branch_id,
                    descendant_id,
                );

                values.push(val);
            }

            Node::statement_batch()
                .chunked_statements(
                    db_session,
                    Node::PULL_ANCESTOR_IDS_QUERY,
                    values,
                    crate::constants::BATCH_CHUNK_SIZE,
                )
                .await
                .map_err(|err| {
                    error!("pull_removed_ancestors_from_descendants: {:?}", err);
                    return err;
                })?;
        }

        Ok(())
    }

    async fn undo_pull_removed_ancestors_from_descendants(
        &mut self,
        db_session: &CachingSession,
    ) -> Result<(), NodecosmosError> {
        if self.reorder_data.parent_changed() {
            let mut values = vec![];

            for descendant_id in &self.reorder_data.descendant_ids {
                let val = (
                    &self.reorder_data.removed_ancestor_ids,
                    self.reorder_data.branch_id,
                    descendant_id,
                );

                values.push(val);
            }

            Node::statement_batch()
                .chunked_statements(
                    db_session,
                    Node::PUSH_ANCESTOR_IDS_QUERY,
                    values,
                    crate::constants::BATCH_CHUNK_SIZE,
                )
                .await
                .map_err(|err| {
                    error!("undo_pull_removed_ancestors_from_descendants: {:?}", err);
                    return err;
                })?;
        }

        Ok(())
    }

    async fn push_added_ancestors_to_node(&mut self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        if self.reorder_data.parent_changed() {
            self.reorder_data
                .node
                .push_ancestor_ids(&self.reorder_data.added_ancestor_ids)
                .execute(db_session)
                .await?;
        }

        Ok(())
    }

    async fn undo_push_added_ancestors_to_node(&mut self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        if self.reorder_data.parent_changed() {
            self.reorder_data
                .node
                .pull_ancestor_ids(&self.reorder_data.added_ancestor_ids)
                .execute(db_session)
                .await?;
        }

        Ok(())
    }

    async fn push_added_ancestors_to_descendants(
        &mut self,
        db_session: &CachingSession,
    ) -> Result<(), NodecosmosError> {
        if self.reorder_data.parent_changed() {
            let mut values = vec![];

            for descendant_id in &self.reorder_data.descendant_ids {
                let val = (
                    &self.reorder_data.added_ancestor_ids,
                    self.reorder_data.branch_id,
                    descendant_id,
                );

                values.push(val);
            }

            Node::statement_batch()
                .chunked_statements(
                    db_session,
                    Node::PUSH_ANCESTOR_IDS_QUERY,
                    values,
                    crate::constants::BATCH_CHUNK_SIZE,
                )
                .await
                .map_err(|err| {
                    error!("push_added_ancestors_to_descendants: {:?}", err);
                    return err;
                })?;
        }

        Ok(())
    }

    async fn undo_push_added_ancestors_to_descendants(
        &mut self,
        db_session: &CachingSession,
    ) -> Result<(), NodecosmosError> {
        if self.reorder_data.parent_changed() {
            let mut values = vec![];

            for descendant_id in &self.reorder_data.descendant_ids {
                let val = (
                    &self.reorder_data.added_ancestor_ids,
                    self.reorder_data.branch_id,
                    descendant_id,
                );

                values.push(val);
            }

            Node::statement_batch()
                .chunked_statements(
                    db_session,
                    Node::PULL_ANCESTOR_IDS_QUERY,
                    values,
                    crate::constants::BATCH_CHUNK_SIZE,
                )
                .await
                .map_err(|err| {
                    error!("undo_push_added_ancestors_to_descendants: {:?}", err);
                    return err;
                })?;
        }

        Ok(())
    }

    async fn update_branch(&mut self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        if self.reorder_data.is_branch() {
            Branch::update(
                db_session,
                self.reorder_data.branch_id,
                BranchUpdate::ReorderNode(BranchReorderData {
                    id: self.reorder_data.node.id,
                    new_parent_id: self.reorder_data.new_parent_id,
                    new_upper_sibling_id: self.reorder_data.new_upper_sibling_id,
                    new_lower_sibling_id: self.reorder_data.new_lower_sibling_id,
                    old_parent_id: self.reorder_data.old_parent_id,
                    old_order_index: self.reorder_data.old_order_index,
                }),
            )
            .await?;
        }

        Ok(())
    }

    async fn undo_update_branch(&mut self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        if self.reorder_data.is_branch() {
            Branch::update(
                db_session,
                self.reorder_data.branch_id,
                BranchUpdate::ReorderNode(BranchReorderData {
                    id: self.reorder_data.node.id,
                    new_parent_id: self.reorder_data.old_parent_id,
                    new_upper_sibling_id: self.reorder_data.new_upper_sibling_id,
                    new_lower_sibling_id: self.reorder_data.new_lower_sibling_id,
                    old_parent_id: self.reorder_data.new_parent_id,
                    old_order_index: self.reorder_data.old_order_index,
                }),
            )
            .await?;
        }

        Ok(())
    }

    fn serialize_and_store_to_disk(&self) {
        // serialize reorder and store to disk
        let serialized = serde_json::to_string(self).expect("Failed to serialize branch merge data");
        let filename = format!("{}{}.json", RECOVER_FILE_PREFIX, self.reorder_data.node.id);
        let path = format!("{}/{}", RECOVERY_DATA_DIR, filename);
        let res = std::fs::write(path.clone(), serialized);

        match res {
            Ok(_) => warn!("Merge Recovery data saved to file: {}", path),
            Err(err) => error!("Error in saving recovery data: {}", err),
        }
    }
}

impl Node {
    pub async fn reorder(&self, data: &RequestData, params: ReorderParams) -> Result<(), NodecosmosError> {
        let reorder_data = ReorderData::from_params(&params, data).await?;

        ReorderValidator::new(&reorder_data).validate()?;
        Reorder::new(reorder_data).run(data.db_session()).await?;

        Ok(())
    }
}
