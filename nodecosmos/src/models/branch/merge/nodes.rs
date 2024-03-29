use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::branch::Branch;
use crate::models::description::{find_description, Description, ObjectType};
use crate::models::node::reorder::ReorderParams;
use crate::models::node::sort::SortNodes;
use crate::models::node::{find_update_title_node, Node, PkNode, UpdateTitleNode};
use crate::models::traits::context::{Context, ModelContext};
use crate::models::traits::ref_cloned::RefCloned;
use crate::models::traits::{Branchable, GroupById, GroupByObjId, Pluck};
use crate::models::udts::{BranchReorderData, TextChange};
use charybdis::operations::{DeleteWithCallbacks, Find, InsertWithCallbacks, UpdateWithCallbacks};
use charybdis::types::{Frozen, List, Uuid};
use log::{error, warn};
use scylla::CachingSession;
use std::collections::HashMap;

pub struct MergeNodes<'a> {
    branch: &'a Branch,
    restored_nodes: Option<Vec<Node>>,
    created_nodes: Option<Vec<Node>>,
    reordered_nodes_data: Option<Vec<&'a BranchReorderData>>,
    edited_title_nodes: Option<Vec<UpdateTitleNode>>,
    edited_node_descriptions: Option<Vec<Description>>,
    original_title_nodes: Option<HashMap<Uuid, UpdateTitleNode>>,
    original_nodes_descriptions: Option<HashMap<Uuid, Description>>,
    title_change_by_object: Option<HashMap<Uuid, TextChange>>,
    description_change_by_object: Option<HashMap<Uuid, TextChange>>,
}

impl<'a> MergeNodes<'a> {
    pub async fn restored_nodes(
        branch: &Branch,
        db_session: &CachingSession,
    ) -> Result<Option<Vec<Node>>, NodecosmosError> {
        if let Some(restored_node_ids) = &branch.restored_nodes {
            let mut branched_nodes = Node::find_by_ids_and_branch_id(db_session, &restored_node_ids, branch.id).await?;
            let already_restored_ids = PkNode::find_by_ids(db_session, &branched_nodes.pluck_id())
                .await?
                .pluck_id_set();

            branched_nodes.retain(|branched_node| !already_restored_ids.contains(&branched_node.id));

            branched_nodes.sort_by_depth();

            return Ok(Some(branched_nodes));
        }

        Ok(None)
    }

    pub async fn created_nodes(
        branch: &Branch,
        db_session: &CachingSession,
    ) -> Result<Option<Vec<Node>>, NodecosmosError> {
        if let Some(created_node_ids) = &branch.created_nodes {
            let mut created_nodes = Node::find_by_ids_and_branch_id(db_session, &created_node_ids, branch.id).await?;

            created_nodes.sort_by_depth();

            return Ok(Some(created_nodes));
        }

        Ok(None)
    }

    pub fn reordered_nodes_data(branch: &Branch) -> Option<List<Frozen<&BranchReorderData>>> {
        if let Some(reordered_nodes) = &branch.reordered_nodes {
            Some(
                reordered_nodes
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
                    .collect(),
            )
        } else {
            None
        }
    }

    pub async fn edited_title_nodes(
        branch: &Branch,
        db_session: &CachingSession,
    ) -> Result<Option<Vec<UpdateTitleNode>>, NodecosmosError> {
        if let Some(edited_title_nodes) = &branch.edited_title_nodes {
            let nodes = find_update_title_node!("branch_id = ? AND id IN ?", (branch.id, edited_title_nodes))
                .execute(db_session)
                .await?
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

    pub async fn edited_node_descriptions(
        branch: &Branch,
        db_session: &CachingSession,
    ) -> Result<Option<Vec<Description>>, NodecosmosError> {
        if let Some(edited_description_nodes) = &branch.edited_description_nodes {
            let descriptions = find_description!(
                "branch_id = ? AND object_id IN ?",
                (branch.id, edited_description_nodes)
            )
            .execute(db_session)
            .await?
            .try_collect()
            .await?;

            return Ok(Some(
                branch.map_original_objects(ObjectType::Node, descriptions).collect(),
            ));
        }

        Ok(None)
    }

    async fn original_title_nodes(
        db_session: &CachingSession,
        branch: &Branch,
    ) -> Result<Option<HashMap<Uuid, UpdateTitleNode>>, NodecosmosError> {
        if let Some(ids) = &branch.edited_title_nodes {
            let nodes_by_id = find_update_title_node!("id IN ? AND branch_id IN ?", (ids, ids))
                .execute(db_session)
                .await?
                .group_by_id()
                .await?;

            return Ok(Some(nodes_by_id));
        }

        Ok(None)
    }

    async fn original_nodes_description(
        db_session: &CachingSession,
        branch: &Branch,
    ) -> Result<Option<HashMap<Uuid, Description>>, NodecosmosError> {
        if let Some(ids) = &branch.edited_description_nodes {
            let nodes_by_id = find_description!("object_id IN ? AND branch_id IN ?", (ids, ids))
                .execute(db_session)
                .await?
                .group_by_obj_id()
                .await?;

            return Ok(Some(nodes_by_id));
        }

        Ok(None)
    }

    pub async fn new(branch: &'a Branch, data: &RequestData) -> Result<Self, NodecosmosError> {
        let restored_nodes = Self::restored_nodes(&branch, data.db_session()).await?;
        let created_nodes = Self::created_nodes(&branch, data.db_session()).await?;
        let reordered_nodes_data = Self::reordered_nodes_data(&branch);
        let edited_title_nodes = Self::edited_title_nodes(&branch, data.db_session()).await?;
        let edited_node_descriptions = Self::edited_node_descriptions(&branch, data.db_session()).await?;
        let original_title_nodes = Self::original_title_nodes(data.db_session(), &branch).await?;
        let original_nodes_descriptions = Self::original_nodes_description(data.db_session(), &branch).await?;

        Ok(Self {
            branch,
            restored_nodes,
            created_nodes,
            reordered_nodes_data,
            edited_title_nodes,
            edited_node_descriptions,
            original_title_nodes,
            original_nodes_descriptions,
            title_change_by_object: None,
            description_change_by_object: None,
        })
    }

    async fn insert_nodes(
        data: &RequestData,
        branch: &Branch,
        merge_nodes: &mut Option<Vec<Node>>,
    ) -> Result<(), NodecosmosError> {
        let branch_node = branch.node(data.db_session()).await?;

        if let Some(merge_nodes) = merge_nodes {
            for merge_node in merge_nodes {
                merge_node.set_merge_context();
                merge_node.set_original_id();
                merge_node.owner_id = branch_node.owner_id;
                merge_node.editor_ids = branch_node.editor_ids.clone();
                merge_node.insert_cb(data).execute(data.db_session()).await?;
            }
        }

        Ok(())
    }

    pub async fn delete_inserted_nodes(
        data: &RequestData,
        merge_nodes: &mut Option<Vec<Node>>,
    ) -> Result<(), NodecosmosError> {
        if let Some(merge_nodes) = merge_nodes {
            for merge_node in merge_nodes {
                merge_node.set_merge_context();
                merge_node.set_original_id();
                merge_node.delete_cb(data).execute(data.db_session()).await?;
            }
        }

        Ok(())
    }

    pub async fn restore_nodes(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        Self::insert_nodes(data, &self.branch, &mut self.restored_nodes).await?;

        Ok(())
    }

    pub async fn undo_restore_nodes(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        Self::delete_inserted_nodes(data, &mut self.restored_nodes).await?;

        Ok(())
    }

    pub async fn create_nodes(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        Self::insert_nodes(data, &self.branch, &mut self.created_nodes).await?;

        Ok(())
    }

    pub async fn undo_create_nodes(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        Self::delete_inserted_nodes(data, &mut self.created_nodes).await?;

        Ok(())
    }

    pub async fn delete_nodes(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        let deleted_node_ids = self.branch.deleted_nodes.ref_cloned();

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

                        deleted_node.set_merge_context();
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

    pub async fn undo_delete_nodes(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        let deleted_node_ids = self.branch.deleted_nodes.ref_cloned();

        for deleted_node_id in deleted_node_ids.clone() {
            let deleted_node = Node::find_by_id_and_branch_id(deleted_node_id, deleted_node_id)
                .execute(data.db_session())
                .await
                .ok();

            match deleted_node {
                Some(mut deleted_node) => {
                    deleted_node.set_original_id();
                    deleted_node.set_merge_context();
                    deleted_node.insert_cb(data).execute(data.db_session()).await?;
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

    pub async fn reorder_nodes(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        if let Some(reordered_nodes_data) = &self.reordered_nodes_data {
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

    pub async fn undo_reorder_nodes(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        if let Some(reordered_nodes_data) = &self.reordered_nodes_data {
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

    pub async fn update_nodes_titles(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        let original_title_nodes = match self.original_title_nodes.as_ref() {
            Some(nodes) => nodes,
            None => return Ok(()),
        };

        let edited_node_titles = match self.edited_title_nodes.as_mut() {
            Some(titles) => titles,
            None => return Ok(()),
        };

        for mut edited_node_title in edited_node_titles {
            if let Some(original_node) = original_title_nodes.get(&edited_node_title.id) {
                if original_node.title != edited_node_title.title {
                    let mut text_change = TextChange::new();
                    text_change.assign_old(Some(original_node.title.clone()));
                    text_change.assign_new(Some(edited_node_title.title.clone()));

                    edited_node_title.set_original_id();
                    edited_node_title.update_cb(data).execute(data.db_session()).await?;

                    self.title_change_by_object
                        .get_or_insert_with(HashMap::default)
                        .insert(edited_node_title.id, text_change);
                }
            }
        }

        Ok(())
    }

    pub async fn undo_update_nodes_titles(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        if let Some(original_title_nodes) = &mut self.original_title_nodes {
            for original_title_node in original_title_nodes.values_mut() {
                original_title_node.update_cb(data).execute(data.db_session()).await?;
            }
        }

        Ok(())
    }

    pub async fn update_nodes_description(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        let edited_node_descriptions = match self.edited_node_descriptions.as_mut() {
            Some(descriptions) => descriptions,
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
            self.description_change_by_object
                .get_or_insert_with(HashMap::default)
                .insert(object_id, text_change);
        }

        Ok(())
    }

    pub async fn undo_update_nodes_description(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
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
