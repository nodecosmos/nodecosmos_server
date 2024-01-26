mod description;

use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::branch::merge::description::DescriptionMerge;
use crate::models::branch::Branch;
use crate::models::node::reorder::ReorderParams;
use crate::models::node::{Node, UpdateDescriptionNode};
use crate::models::udts::{Conflict, ConflictStatus};
use crate::utils::cloned_ref::ClonedRef;
use crate::utils::logger::{log_error, log_warning};
use charybdis::operations::{DeleteWithExtCallbacks, Find, InsertWithExtCallbacks, UpdateWithExtCallbacks};
use charybdis::types::Uuid;

impl Branch {
    pub async fn merge(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        self.validate_no_existing_conflicts().await?;
        self.check_conflicts(data.db_session()).await?;
        self.restore_nodes(data).await?;
        self.create_nodes(data).await?;
        self.delete_nodes(data).await?;
        self.update_nodes_titles(data).await?;
        self.update_nodes_description(data).await?;

        Ok(())
    }

    async fn restore_nodes(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        let restored_nodes = self.restored_nodes(data.db_session()).await?;

        self.insert_nodes(data, restored_nodes).await
    }

    async fn create_nodes(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        let created_nodes = self.created_nodes(data.db_session()).await?;

        self.insert_nodes(data, created_nodes).await
    }

    async fn insert_nodes(
        &mut self,
        data: &RequestData,
        merge_nodes: Option<Vec<Node>>,
    ) -> Result<(), NodecosmosError> {
        match self.node(data.db_session()).await? {
            Some(node) => {
                if let Some(merge_nodes) = merge_nodes {
                    for mut merge_node in merge_nodes {
                        merge_node.merge = true;
                        merge_node.branch_id = merge_node.id;
                        merge_node.owner_id = node.owner_id;
                        merge_node.owner_type = node.owner_type.clone();
                        merge_node.editor_ids = node.editor_ids.clone();
                        merge_node.insert_cb(data.db_session(), data).await?;
                    }
                }
            }
            None => {
                return Err(NodecosmosError::InternalServerError(format!(
                    "Node with id {} not found",
                    self.node_id
                )))
            }
        }
        Ok(())
    }

    async fn delete_nodes(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        let deleted_node_ids = self.deleted_nodes.cloned_ref();

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

    async fn reorder_nodes(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        let deleted_node_ids = self.deleted_nodes.cloned_ref();
        let created_node_ids = self.created_nodes.cloned_ref();

        if let Some(reordered_nodes) = &self.reordered_nodes {
            for reorder_node_data in reordered_nodes {
                if deleted_node_ids.contains(&reorder_node_data.id) || created_node_ids.contains(&reorder_node_data.id)
                {
                    continue;
                };

                let node =
                    Node::find_by_primary_key_value(data.db_session(), (reorder_node_data.id, reorder_node_data.id))
                        .await?;

                let reorder_params = ReorderParams {
                    id: reorder_node_data.id,
                    branch_id: reorder_node_data.id,
                    new_parent_id: reorder_node_data.new_parent_id,
                    new_upper_sibling_id: reorder_node_data.new_upper_sibling_id,
                    new_lower_sibling_id: reorder_node_data.new_lower_sibling_id,
                };
                node.reorder(data, reorder_params).await?;
            }
        }
        Ok(())
    }

    async fn update_nodes_titles(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        if let Some(edited_node_titles) = self.edited_title_nodes(data.db_session()).await? {
            for mut edited_node_title in edited_node_titles {
                edited_node_title.branch_id = edited_node_title.id;
                edited_node_title.update_cb(data.db_session(), data).await?;
            }
        }

        Ok(())
    }

    async fn update_nodes_description(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        let created_node_ids = &self.created_nodes.cloned_ref();
        let deleted_node_ids = self.deleted_nodes.cloned_ref();

        if let Some(edited_description_nodes) = self.edited_description_nodes(data.db_session()).await? {
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

                if let (Some(original_base64), Some(new_base64)) =
                    (original.description_base64, edited_description_node.description_base64)
                {
                    let description_merge = DescriptionMerge::new(original_base64, new_base64);
                    original.description = Some(description_merge.html);
                    original.description_markdown = Some(description_merge.markdown);
                    original.description_base64 = Some(description_merge.base64);
                    original.short_description = Some(description_merge.short_description);

                    original.update_cb(data.db_session(), data).await?;
                }
            }
        }

        Ok(())
    }
}
