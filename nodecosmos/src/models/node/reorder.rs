use charybdis::batch::ModelBatch;
use charybdis::operations::Update;
use charybdis::types::{Double, Uuid};
use scylla::CachingSession;
use serde::{Deserialize, Serialize};

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
use crate::models::recovery::{RecoveryLog, RecoveryObjectType};
use crate::models::traits::Branchable;
use crate::models::udts::BranchReorderData;

pub mod data;
mod validator;

#[derive(Clone, Copy, Serialize, Deserialize, PartialOrd, PartialEq, Debug)]
pub enum ReorderStep {
    BeforeStart = -1,
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
    AfterFinish = 12,
}

impl ReorderStep {
    pub fn increment(&mut self) {
        *self = ReorderStep::from(*self as i8 + 1);
    }

    pub fn decrement(&mut self) {
        *self = ReorderStep::from(*self as i8 - 1);
    }
}

impl From<i8> for ReorderStep {
    fn from(value: i8) -> Self {
        match value {
            -1 => ReorderStep::BeforeStart,
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
            12 => ReorderStep::AfterFinish,
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
    reorder_data: ReorderData,
    reorder_step: ReorderStep,
}

impl Reorder {
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
            log::error!(
                "Reorder failed for node: {}\n! ERROR: {:?}",
                self.reorder_data.node.id,
                err
            );

            self.recover(db_session).await.map_err(|recover_err| {
                log::error!(
                    "Fatal Reorder recovery failed for node: {}\n! ERROR: {:?}",
                    self.reorder_data.node.id,
                    recover_err
                );

                NodecosmosError::FatalReorderError(err.to_string())
            })?;

            return Err(err);
        }

        Ok(())
    }

    async fn execute_reorder(&mut self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        while self.reorder_step <= ReorderStep::Finish {
            // log current step
            if self.reorder_step > ReorderStep::Start && self.reorder_step < ReorderStep::Finish {
                self.update_recovery_log_step(db_session, self.reorder_step as i8)
                    .await?;
            }

            match self.reorder_step {
                ReorderStep::BeforeStart => {
                    log::error!("should not execute before placeholder");
                }
                ReorderStep::Start => {
                    self.create_recovery_log(db_session).await?;
                }
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
                ReorderStep::Finish => {
                    self.delete_recovery_log(db_session).await?;
                }
                ReorderStep::AfterFinish => {
                    log::error!("should not execute after placeholder");
                }
            }

            self.reorder_step.increment();
        }

        Ok(())
    }

    async fn recover(&mut self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        while self.reorder_step >= ReorderStep::Start {
            // log current step
            if self.reorder_step > ReorderStep::Start && self.reorder_step < ReorderStep::Finish {
                self.update_recovery_log_step(db_session, self.reorder_step as i8)
                    .await?;
            }

            match self.reorder_step {
                ReorderStep::BeforeStart => {
                    log::error!("should not hit before placeholder");
                }
                ReorderStep::Start => {
                    self.delete_recovery_log(db_session).await?;
                }
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
                ReorderStep::Finish => {
                    log::error!("should not recover finished process");
                }
                ReorderStep::AfterFinish => {
                    log::error!("should not hit after placeholder");
                }
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
                log::error!("remove_node_from_removed_ancestors: {:?}", err);
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
                log::error!("undo_remove_node_from_old_ancestors: {:?}", err);
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
                    log::error!("delete_node_descendants_from_removed_ancestors: {:?}", err);
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
                    log::error!("undo_delete_node_descendants_from_removed_ancestors: {:?}", err);
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
                log::error!("add_node_to_new_ancestors: {:?}", err);
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
                log::error!("undo_add_node_to_new_ancestors: {:?}", err);
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
                    log::error!("insert_node_descendants_to_added_ancestors: {:?}", err);
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
                    log::error!("undo_insert_node_descendants_to_added_ancestors: {:?}", err);
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
                    log::error!("pull_removed_ancestors_from_descendants: {:?}", err);
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
                    log::error!("undo_pull_removed_ancestors_from_descendants: {:?}", err);
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
                    log::error!("push_added_ancestors_to_descendants: {:?}", err);
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
                    log::error!("undo_push_added_ancestors_to_descendants: {:?}", err);
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
}

impl RecoveryLog<'_> for Reorder {
    fn rec_id(&self) -> Uuid {
        self.reorder_data.node.id
    }

    fn rec_branch_id(&self) -> Uuid {
        self.reorder_data.node.branch_id
    }

    fn rec_object_type(&self) -> RecoveryObjectType {
        RecoveryObjectType::Reorder
    }

    fn set_step(&mut self, step: i8) {
        self.reorder_step = ReorderStep::from(step);
    }

    async fn recover_from_log(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        self.recover(data.db_session()).await.map_err(|recover_err| {
            log::error!(
                "Fatal Reorder Error: recover_from_log failed for node: {}\n! ERROR: {:?}",
                self.reorder_data.node.id,
                recover_err
            );

            NodecosmosError::FatalReorderError(recover_err.to_string())
        })?;

        let _ = self.unlock_resource(data).await.map_err(|unlock_err| {
            log::error!(
                "Reorder Error unlock_resource failed for node: {}\n! ERROR: {:?}",
                self.reorder_data.node.id,
                unlock_err
            );
            NodecosmosError::FatalReorderError(unlock_err.to_string())
        });

        Ok(())
    }
}

impl Node {
    pub async fn reorder(data: &RequestData, params: &ReorderParams) -> Result<(), NodecosmosError> {
        let reorder_data = ReorderData::from_params(params, data).await?;

        ReorderValidator::new(&reorder_data).validate()?;
        Reorder::new(reorder_data).run(data.db_session()).await?;

        Ok(())
    }
}
