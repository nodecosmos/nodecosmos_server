mod description;

use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::branch::merge::description::DescriptionMerge;
use crate::models::branch::Branch;
use crate::models::node::{Node, UpdateDescriptionNode};
use crate::utils::cloned_ref::ClonedRef;
use crate::utils::logger::log_error;
use charybdis::operations::{DeleteWithExtCallbacks, Find, InsertWithExtCallbacks, UpdateWithExtCallbacks};

impl Branch {
    pub async fn merge(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        self.create_nodes(data).await?;
        self.delete_nodes(data).await?;
        self.update_nodes_titles(data).await?;
        self.update_nodes_description(data).await?;

        Ok(())
    }

    async fn create_nodes(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        let deleted_node_ids = self.deleted_nodes.cloned_ref();
        let created_nodes = self.created_nodes(data.db_session()).await?;

        match self.node(data.db_session()).await? {
            Some(node) => {
                if let Some(created_nodes) = created_nodes {
                    for mut created_node in created_nodes {
                        if deleted_node_ids.contains(&created_node.id) {
                            continue;
                        }

                        created_node.merge = true;
                        created_node.branch_id = created_node.id;
                        created_node.owner_id = node.owner_id;
                        created_node.owner_type = node.owner_type.clone();
                        created_node.editor_ids = node.editor_ids.clone();
                        created_node.insert_cb(data.db_session(), data).await?;
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

        for deleted_node_id in deleted_node_ids {
            let mut deleted_node =
                Node::find_by_primary_key_value(data.db_session(), (deleted_node_id, deleted_node_id)).await?;

            deleted_node.delete_cb(data.db_session(), data).await?;
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
            for mut edited_description_node in edited_description_nodes {
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
                    let mut description_merge = DescriptionMerge::new(original_base64, new_base64);
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
