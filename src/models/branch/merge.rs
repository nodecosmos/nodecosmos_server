use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::branch::Branch;
use crate::models::node::Node;
use crate::utils::cloned_ref::ClonedRef;
use charybdis::operations::{DeleteWithExtCallbacks, Find, InsertWithExtCallbacks};

impl Branch {
    pub async fn merge(data: &RequestData, branch: &mut Branch) -> Result<(), NodecosmosError> {
        branch.create_nodes(data).await?;
        branch.delete_nodes(data).await?;
        branch.update_nodes_titles(data).await?;
        branch.update_nodes_description(data).await?;

        Ok(())
    }

    pub async fn create_nodes(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        let deleted_node_ids = self.deleted_nodes.cloned_ref();
        let created_nodes = self.created_nodes(data.db_session()).await?;

        match self.node(data.db_session()).await? {
            Some(node) => {
                if let Some(created_nodes) = created_nodes {
                    for mut created_node in created_nodes {
                        if deleted_node_ids.contains(&created_node.id) {
                            continue;
                        }

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

    pub async fn delete_nodes(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        let deleted_node_ids = self.deleted_nodes.cloned_ref();

        for deleted_node_id in deleted_node_ids {
            let mut deleted_node =
                Node::find_by_primary_key_value(data.db_session(), (deleted_node_id, deleted_node_id)).await?;

            deleted_node.delete_cb(data.db_session(), data).await?;
        }

        Ok(())
    }

    pub async fn update_nodes_titles(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        if let Some(edited_node_titles) = self.edited_title_nodes(data.db_session()).await? {
            for mut edited_node_title in edited_node_titles {
                edited_node_title.branch_id = edited_node_title.id;
                edited_node_title.insert_cb(data.db_session(), data).await?;
            }
        }

        Ok(())
    }

    pub async fn update_nodes_description(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        if let Some(edited_node_descriptions) = self.edited_description_nodes(data.db_session()).await? {
            for mut edited_node_description in edited_node_descriptions {
                edited_node_description.branch_id = edited_node_description.id;
                edited_node_description.insert_cb(data.db_session(), data).await?;
            }
        }

        Ok(())
    }
}
