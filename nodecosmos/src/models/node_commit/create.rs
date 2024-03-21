use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::node::reorder::data::ReorderData;
use crate::models::node::Node;
use crate::models::node_commit::reorder::ReorderCommit;
use crate::models::node_commit::NodeCommit;
use crate::models::node_descendants_commit::NodeDescendantsCommit;
use crate::models::node_tree_position_commit::NodeTreePositionCommit;
use crate::models::traits::VersionedNodePluck;
use charybdis::batch::ModelBatch;
use charybdis::operations::Insert;
use charybdis::types::{Double, Uuid};
use chrono::Utc;
use log::error;
use scylla::CachingSession;
use std::collections::HashMap;

pub struct TreePositionChange {
    pub parent_id: Option<Uuid>,
    pub order_index: Option<Double>,
    pub removed_ancestor_ids: Option<Vec<Uuid>>,
    pub added_ancestor_ids: Option<Vec<Uuid>>,
}

type RemovedIds = Vec<Uuid>;
pub(crate) type NewDescendantCommitById = HashMap<Uuid, Uuid>;

#[allow(unused)]
pub enum NodeChange {
    Title(String),
    Description(Uuid),
    Workflow(Uuid),
    TreePosition(TreePositionChange),
    Descendants(Option<RemovedIds>, Option<NewDescendantCommitById>),
}

impl NodeCommit {
    pub async fn handle_creation(
        db_session: &CachingSession,
        node: &Node,
        user_id: Uuid,
    ) -> Result<(), NodecosmosError> {
        let tree_position_commit = NodeTreePositionCommit::from_node(node);
        tree_position_commit.insert().execute(db_session).await?;

        let now = Utc::now();

        let mut node_commit = NodeCommit {
            node_id: node.id,
            branch_id: node.branch_id,
            created_at: now,
            id: Uuid::new_v4(),
            prev_commit_id: None,
            prev_commit_branch_id: None,
            user_id: Some(user_id),
            node_title: node.title.clone(),
            node_creator_id: node.creator_id,
            node_created_at: node.created_at,
            description_commit_id: None,
            node_tree_position_commit_id: tree_position_commit.id,
            node_descendants_commit_id: None,
            workflow_commit_id: None,
        };

        node_commit.insert().execute(db_session).await?;
        node_commit.create_ancestors_commits(db_session).await?;

        Ok(())
    }

    pub async fn handle_change(
        db_session: &CachingSession,
        node_id: Uuid,
        branch_id: Uuid,
        user_id: Uuid,
        changes: &Vec<NodeChange>,
        create_ancestors_commits: bool,
    ) -> Result<NodeCommit, NodecosmosError> {
        let mut new_node_commit = NodeCommit::init_from_latest(db_session, node_id, branch_id, user_id).await?;

        for attribute in changes {
            match attribute {
                NodeChange::Title(title) => {
                    new_node_commit.node_title = title.clone();
                }
                NodeChange::Description(description_commit_id) => {
                    new_node_commit.description_commit_id = Some(*description_commit_id);
                }
                NodeChange::Workflow(workflow_commit_id) => {
                    new_node_commit.workflow_commit_id = Some(*workflow_commit_id);
                }
                NodeChange::TreePosition(tree_position_change) => {
                    let old_vtp = new_node_commit.tree_position_commit(db_session).await?;
                    let mut new_vtp = NodeTreePositionCommit::init_from(&old_vtp);

                    if let Some(parent_id) = tree_position_change.parent_id {
                        new_vtp.parent_id = Some(parent_id);
                    }

                    if let Some(order_index) = tree_position_change.order_index {
                        new_vtp.order_index = order_index;
                    }

                    if let Some(removed_ancestor_ids) = &tree_position_change.removed_ancestor_ids {
                        let mut ancestor_ids = new_vtp.ancestor_ids.unwrap_or_default();
                        ancestor_ids.retain(|id| !removed_ancestor_ids.contains(id));
                        new_vtp.ancestor_ids = Some(ancestor_ids);
                    }

                    if let Some(added_ancestor_ids) = &tree_position_change.added_ancestor_ids {
                        let mut ancestor_ids = new_vtp.ancestor_ids.unwrap_or_default();
                        ancestor_ids.extend(added_ancestor_ids);
                        new_vtp.ancestor_ids = Some(ancestor_ids);
                    }
                }
                NodeChange::Descendants(removed_ids, new_descendant_node_commit_id_by_id) => {
                    let old_ndc = new_node_commit.node_descendants_commit(db_session).await?;
                    if let Some(old_node_descendant_commit) = old_ndc {
                        let mut descendant_node_commit_id_by_id =
                            old_node_descendant_commit.descendant_node_commit_id_by_id;

                        if let Some(remove_ids) = removed_ids {
                            for id in remove_ids {
                                descendant_node_commit_id_by_id.remove(id);
                            }
                        }

                        if let Some(new_descendant_node_commit_id_by_id) = new_descendant_node_commit_id_by_id {
                            descendant_node_commit_id_by_id.extend(new_descendant_node_commit_id_by_id);
                        }

                        let new_node_descendants_commit = NodeDescendantsCommit {
                            id: Uuid::new_v4(),
                            node_id,
                            descendant_node_commit_id_by_id,
                        };

                        new_node_descendants_commit.insert().execute(db_session).await?;
                        new_node_commit.node_descendants_commit_id = Some(new_node_descendants_commit.id);
                    }
                }
            }
        }

        new_node_commit.insert().execute(db_session).await?;

        if create_ancestors_commits {
            new_node_commit.create_ancestors_commits(db_session).await?;
        }

        Ok(new_node_commit)
    }

    pub async fn handle_deletion(data: &RequestData, node: &Node) -> Result<(), NodecosmosError> {
        let mut node_commit = NodeCommit::find_latest(data.db_session(), &node.id, &node.branch_id).await?;

        node_commit
            .create_ancestors_commits_for_deleted_node(data.db_session())
            .await?;

        Ok(())
    }

    pub async fn handle_reorder(data: &RequestData, reorder_data: &ReorderData) {
        let reorder_commit = ReorderCommit::from_reorder_data(data.db_session(), data.current_user_id(), &reorder_data);
        let res = reorder_commit.create().await;
        match res {
            Ok(_) => {}
            Err(e) => {
                error!("handle_reorder: {:?}", e)
            }
        }
    }

    async fn create_ancestors_commits(&mut self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        let versioned_ancestors = self.latest_ancestor_commits(db_session).await?;
        let vnd_ids = versioned_ancestors.pluck_node_descendants_commit_id();
        let grouped_v_node_descendants = NodeDescendantsCommit::find_grouped_by_node_id(db_session, vnd_ids).await?;
        let mut new_v_node_descendants = vec![];
        let mut new_v_nodes = vec![];

        for versioned_ancestor in versioned_ancestors {
            let default_v_node_descendants = Default::default();
            let v_node_descendant = grouped_v_node_descendants
                .get(&versioned_ancestor.node_id)
                .unwrap_or_else(|| &default_v_node_descendants);

            let mut descendant_node_commit_id_by_id = v_node_descendant.descendant_node_commit_id_by_id.clone();
            descendant_node_commit_id_by_id.insert(self.node_id, self.id);

            let new_v_node_descendant =
                NodeDescendantsCommit::new(versioned_ancestor.node_id, descendant_node_commit_id_by_id);
            let mut new_v_node = NodeCommit::init_from(&versioned_ancestor, self.branch_id, self.user_id);
            new_v_node.node_descendants_commit_id = Some(new_v_node_descendant.id);

            new_v_node_descendants.push(new_v_node_descendant);
            new_v_nodes.push(new_v_node);
        }

        NodeDescendantsCommit::unlogged_batch()
            .chunked_insert(db_session, &new_v_node_descendants, 100)
            .await?;

        NodeCommit::unlogged_batch()
            .chunked_insert(db_session, &new_v_nodes, 100)
            .await?;

        Ok(())
    }

    async fn create_ancestors_commits_for_deleted_node(
        &mut self,
        db_session: &CachingSession,
    ) -> Result<(), NodecosmosError> {
        let mut new_v_node_descendants = vec![];
        let mut new_v_nodes = vec![];

        let ancestors = self.latest_ancestor_commits(db_session).await?;
        let vnd_ids = ancestors.pluck_node_descendants_commit_id();
        let grouped_v_node_descendants = NodeDescendantsCommit::find_grouped_by_node_id(db_session, vnd_ids).await?;

        let node_descendant_commit = self.node_descendants_commit(db_session).await?;
        let mut node_descendant_ids_to_remove = vec![self.node_id];

        if let Some(node_descendant_commit) = node_descendant_commit {
            let node_descendant_ids: Vec<Uuid> = node_descendant_commit
                .descendant_node_commit_id_by_id
                .keys()
                .cloned()
                .collect();

            node_descendant_ids_to_remove.extend(node_descendant_ids);
        }

        for ancestor in ancestors {
            let default_v_node_descendants = Default::default();
            let v_node_descendant = grouped_v_node_descendants
                .get(&ancestor.node_id)
                .unwrap_or_else(|| &default_v_node_descendants);

            let mut anc_descendant_node_commit_id_by_id = v_node_descendant.descendant_node_commit_id_by_id.clone();
            node_descendant_ids_to_remove.iter().for_each(|node_id| {
                anc_descendant_node_commit_id_by_id.remove(node_id);
            });

            let new_v_node_descendant =
                NodeDescendantsCommit::new(ancestor.node_id, anc_descendant_node_commit_id_by_id);
            let mut new_v_node = NodeCommit::init_from(&ancestor, self.branch_id, self.user_id);
            new_v_node.node_descendants_commit_id = Some(new_v_node_descendant.id);

            new_v_node_descendants.push(new_v_node_descendant);
            new_v_nodes.push(new_v_node);
        }

        NodeDescendantsCommit::unlogged_batch()
            .chunked_insert(db_session, &new_v_node_descendants, 100)
            .await?;

        NodeCommit::unlogged_batch()
            .chunked_insert(db_session, &new_v_nodes, 100)
            .await?;

        Ok(())
    }
}
