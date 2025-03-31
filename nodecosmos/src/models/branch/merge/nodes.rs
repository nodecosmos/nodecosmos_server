use std::collections::{HashMap, HashSet};

use anyhow::Context;
use charybdis::operations::{DeleteWithCallbacks, Find, InsertWithCallbacks, UpdateWithCallbacks};
use charybdis::types::{Frozen, List, Uuid};
use scylla::client::caching_session::CachingSession;
use serde::{Deserialize, Serialize};

use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::branch::Branch;
use crate::models::node::reorder::ReorderParams;
use crate::models::node::sort::SortNodes;
use crate::models::node::{find_update_title_node, Node, PkNode, UpdateTitleNode};
use crate::models::traits::{Branchable, GroupById, Pluck, WhereInChunksExec};
use crate::models::traits::{ModelContext, ObjectType};
use crate::models::udts::{BranchReorderData, TextChange};

#[derive(Serialize, Deserialize, Default)]
pub struct MergeNodes {
    pub restored_nodes: Option<Vec<Node>>,
    pub created_nodes: Option<Vec<Node>>,
    pub deleted_nodes: Option<Vec<Node>>,
    reordered_nodes_data: Option<Vec<BranchReorderData>>,
    edited_title_nodes: Option<Vec<UpdateTitleNode>>,
    original_title_nodes: Option<HashMap<Uuid, UpdateTitleNode>>,
}

impl MergeNodes {
    pub async fn restored_nodes(
        db_session: &CachingSession,
        branch: &Branch,
    ) -> Result<Option<Vec<Node>>, NodecosmosError> {
        if let Some(restored_node_ids) = &branch.restored_nodes {
            let mut branched_nodes: Vec<Node> =
                Node::find_by_ids(db_session, branch.id, &restored_node_ids.iter().cloned().collect())
                    .await
                    .try_collect()
                    .await?;
            let already_restored_ids =
                PkNode::find_by_ids(db_session, branch.original_id(), &branched_nodes.pluck_id())
                    .await?
                    .pluck_id_set();

            branched_nodes.retain(|branched_node| !already_restored_ids.contains(&branched_node.id));

            branched_nodes.sort_by_depth();

            return Ok(Some(branched_nodes));
        }

        Ok(None)
    }

    pub async fn created_nodes(
        db_session: &CachingSession,
        branch: &Branch,
    ) -> Result<Option<Vec<Node>>, NodecosmosError> {
        if let Some(created_node_ids) = &branch.created_nodes {
            let mut created_node_ids = created_node_ids.iter().cloned().collect::<Vec<Uuid>>();
            if let Some(deleted_node_ids) = &branch.deleted_nodes {
                created_node_ids.retain(|id| !deleted_node_ids.contains(id));
            }

            let n_stream = Node::find_by_ids(db_session, branch.id, &created_node_ids).await;
            let mut created_nodes = branch.filter_out_nodes_with_deleted_parents(n_stream).await?;

            created_nodes.sort_by_depth();

            return Ok(Some(created_nodes));
        }

        Ok(None)
    }

    pub async fn deleted_nodes(
        db_session: &CachingSession,
        branch: &Branch,
    ) -> Result<Option<Vec<Node>>, NodecosmosError> {
        if let Some(deleted_node_ids) = &branch.deleted_nodes {
            let mut deleted_nodes: Vec<Node> = Node::find_by_ids(
                db_session,
                branch.original_id(),
                &deleted_node_ids.iter().cloned().collect(),
            )
            .await
            .try_collect()
            .await?;

            deleted_nodes.sort_by_depth();

            return Ok(Some(deleted_nodes));
        }

        Ok(None)
    }

    pub fn reordered_nodes_data(branch: &Branch) -> Option<List<Frozen<BranchReorderData>>> {
        branch.reordered_nodes.as_ref().map(|reordered_nodes| {
            reordered_nodes
                .clone()
                .into_iter()
                .filter(|reorder_data| {
                    !branch
                        .deleted_nodes
                        .as_ref()
                        .map_or(false, |deleted_nodes| deleted_nodes.contains(&reorder_data.id))
                        && !branch
                            .created_nodes
                            .as_ref()
                            .map_or(false, |created_nodes| created_nodes.contains(&reorder_data.id))
                })
                .collect()
        })
    }

    pub async fn edited_title_nodes(
        db_session: &CachingSession,
        branch: &Branch,
    ) -> Result<Option<Vec<UpdateTitleNode>>, NodecosmosError> {
        if let Some(edited_title_nodes) = &branch.edited_title_nodes {
            let nodes = edited_title_nodes
                .where_in_chunked_query(db_session, |ids_chunk| {
                    find_update_title_node!("branch_id = ? AND id IN ?", (branch.id, ids_chunk))
                })
                .await
                .try_collect()
                .await?;

            let edited_title_nodes = branch
                .map_original_records(nodes, ObjectType::Node)
                .map(|mut edited_title_node| {
                    edited_title_node.set_merge_context();
                    edited_title_node
                })
                .collect();

            return Ok(Some(edited_title_nodes));
        }

        Ok(None)
    }

    async fn original_title_nodes(
        db_session: &CachingSession,
        branch: &Branch,
    ) -> Result<Option<HashMap<Uuid, UpdateTitleNode>>, NodecosmosError> {
        if let Some(ids) = &branch.edited_title_nodes {
            let nodes_by_id = ids
                .where_in_chunked_query(db_session, |ids_chunk| {
                    find_update_title_node!("branch_id = ? AND id IN ?", (branch.original_id(), ids_chunk))
                })
                .await
                .group_by_id()
                .await?;

            return Ok(Some(nodes_by_id));
        }

        Ok(None)
    }

    pub async fn new(db_session: &CachingSession, branch: &Branch) -> Result<Self, NodecosmosError> {
        let restored_nodes = Self::restored_nodes(db_session, branch).await?;
        let created_nodes = Self::created_nodes(db_session, branch).await?;
        let deleted_nodes = Self::deleted_nodes(db_session, branch).await?;
        let reordered_nodes_data = Self::reordered_nodes_data(branch);
        let edited_title_nodes = Self::edited_title_nodes(db_session, branch).await?;
        let original_title_nodes = Self::original_title_nodes(db_session, branch).await?;

        Ok(Self {
            restored_nodes,
            created_nodes,
            deleted_nodes,
            reordered_nodes_data,
            edited_title_nodes,
            original_title_nodes,
        })
    }

    async fn insert_nodes(
        data: &RequestData,
        branch: &mut Branch,
        merge_nodes: &mut Option<Vec<Node>>,
    ) -> Result<(), NodecosmosError> {
        let branch_node = branch.node(data.db_session()).await?;

        if let Some(merge_nodes) = merge_nodes {
            for merge_node in merge_nodes {
                merge_node.set_merge_context();
                merge_node.set_original_id();

                merge_node.is_public = branch_node.is_public;
                merge_node.owner_id = branch_node.owner_id;
                merge_node.editor_ids = branch_node.editor_ids.clone();
                merge_node.viewer_ids = branch_node.viewer_ids.clone();

                merge_node.insert_cb(data).execute(data.db_session()).await?;
            }
        }

        Ok(())
    }

    pub async fn remove_nodes(data: &RequestData, merge_nodes: &mut Option<Vec<Node>>) -> Result<(), NodecosmosError> {
        if let Some(merge_nodes) = merge_nodes {
            let mut deleted_ids = HashSet::new();

            for merge_node in merge_nodes {
                if merge_node
                    .ancestor_ids
                    .as_ref()
                    .is_some_and(|ids| ids.iter().any(|id| deleted_ids.contains(id)))
                {
                    // skip deletion of node if it has an ancestor that is also deleted as
                    // it will be removed in the callback
                    continue;
                }

                merge_node.set_merge_context();
                merge_node.set_original_id();

                merge_node.delete_cb(data).execute(data.db_session()).await?;

                deleted_ids.insert(merge_node.id);
            }
        }

        Ok(())
    }

    pub async fn restore_nodes(&mut self, data: &RequestData, branch: &mut Branch) -> Result<(), NodecosmosError> {
        Self::insert_nodes(data, branch, &mut self.restored_nodes)
            .await
            .context("Failed to restore nodes")?;

        Ok(())
    }

    pub async fn undo_restore_nodes(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        Self::remove_nodes(data, &mut self.restored_nodes)
            .await
            .context("Failed to undo restore nodes")?;

        Ok(())
    }

    pub async fn create_nodes(&mut self, data: &RequestData, branch: &mut Branch) -> Result<(), NodecosmosError> {
        Self::insert_nodes(data, branch, &mut self.created_nodes)
            .await
            .context("Failed to create nodes")?;

        Ok(())
    }

    pub async fn undo_create_nodes(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        Self::remove_nodes(data, &mut self.created_nodes)
            .await
            .context("Failed to undo create nodes")?;

        Ok(())
    }

    pub async fn delete_nodes(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        Self::remove_nodes(data, &mut self.deleted_nodes)
            .await
            .context("Failed to delete inserted nodes")?;

        Ok(())
    }

    pub async fn undo_delete_nodes(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        if let Some(deleted_nodes) = &mut self.deleted_nodes {
            for deleted_node in deleted_nodes {
                deleted_node.set_merge_context();
                deleted_node
                    .insert_cb(data)
                    .execute(data.db_session())
                    .await
                    .context("Failed to insert node")?;
            }
        }

        Ok(())
    }

    pub async fn reorder_nodes(&mut self, data: &RequestData, branch: &Branch) -> Result<(), NodecosmosError> {
        if let Some(reordered_nodes_data) = &self.reordered_nodes_data {
            for reorder_node_data in reordered_nodes_data {
                let node = Node::find_by_primary_key_value((branch.root_id, reorder_node_data.id))
                    .execute(data.db_session())
                    .await;

                match node {
                    Ok(node) => {
                        let reorder_params = ReorderParams {
                            root_id: node.root_id,
                            branch_id: node.original_id(),
                            id: reorder_node_data.id,
                            new_parent_id: reorder_node_data.new_parent_id,
                            new_upper_sibling_id: reorder_node_data.new_upper_sibling_id,
                            new_lower_sibling_id: reorder_node_data.new_lower_sibling_id,
                            new_order_index: None,
                        };

                        // reorder has it's own recovery logic
                        let res = Node::reorder(data, &reorder_params).await;

                        if let Err(e) = res {
                            match e {
                                NodecosmosError::FatalReorderError { .. } => {
                                    log::error!(
                                        "[reorder_nodes] Failed to reorder node with id {} and branch id {}",
                                        reorder_node_data.id,
                                        node.branch_id
                                    );

                                    return Err(e);
                                }
                                _ => {
                                    log::warn!(
                                        "[reorder_nodes] Failed to reorder node with id {} and branch id {}: {:?}",
                                        reorder_node_data.id,
                                        node.branch_id,
                                        e
                                    );
                                }
                            }
                        }
                    }
                    Err(e) => {
                        log::error!(
                            "[reorder_nodes] Failed to find node with id {}: {:?}",
                            reorder_node_data.id,
                            e
                        )
                    }
                }
            }
        }
        Ok(())
    }

    pub async fn undo_reorder_nodes(&mut self, data: &RequestData, branch: &Branch) -> Result<(), NodecosmosError> {
        if let Some(reordered_nodes_data) = &self.reordered_nodes_data {
            for reorder_node_data in reordered_nodes_data {
                let node = Node::find_by_primary_key_value((branch.root_id, reorder_node_data.id))
                    .execute(data.db_session())
                    .await;

                match node {
                    Ok(node) => {
                        let reorder_params = ReorderParams {
                            root_id: node.root_id,
                            branch_id: node.original_id(),
                            id: reorder_node_data.id,
                            new_parent_id: reorder_node_data.old_parent_id,
                            new_order_index: Some(reorder_node_data.old_order_index),
                            new_upper_sibling_id: None,
                            new_lower_sibling_id: None,
                        };

                        let res = Node::reorder(data, &reorder_params).await;

                        if let Err(e) = res {
                            log::error!(
                                "Failed to undo_reorder_nodes: {:?}. node_id: {} branch_id: {}",
                                e,
                                node.id,
                                node.branch_id
                            );
                        }
                    }
                    Err(e) => {
                        log::error!(
                            "[undo_reorder_nodes] Failed to find node with id {}: {:?}",
                            reorder_node_data.id,
                            e
                        )
                    }
                }
            }
        }

        Ok(())
    }

    pub async fn update_title(&mut self, data: &RequestData, branch: &mut Branch) -> Result<(), NodecosmosError> {
        let original_title_nodes = match self.original_title_nodes.as_ref() {
            Some(nodes) => nodes,
            None => return Ok(()),
        };

        let edited_node_titles = match self.edited_title_nodes.as_mut() {
            Some(titles) => titles,
            None => return Ok(()),
        };

        for edited_node_title in edited_node_titles {
            if let Some(original_node) = original_title_nodes.get(&edited_node_title.id) {
                if original_node.title != edited_node_title.title {
                    let mut text_change = TextChange::new();
                    text_change.assign_old(Some(original_node.title.clone()));
                    text_change.assign_new(Some(edited_node_title.title.clone()));

                    edited_node_title.set_original_id();
                    edited_node_title
                        .update_cb(data)
                        .execute(data.db_session())
                        .await
                        .context("Failed to update node title")?;

                    branch
                        .title_change_by_object
                        .get_or_insert_with(HashMap::default)
                        .insert(edited_node_title.id, text_change);
                }
            }
        }

        Ok(())
    }

    pub async fn undo_update_title(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        if let Some(original_title_nodes) = &mut self.original_title_nodes {
            for original_title_node in original_title_nodes.values_mut() {
                original_title_node.update_cb(data).execute(data.db_session()).await?;
            }
        }

        Ok(())
    }
}
