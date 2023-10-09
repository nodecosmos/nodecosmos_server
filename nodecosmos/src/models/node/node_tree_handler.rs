use crate::models::node::{Node, UpdateTitleNode};
use crate::models::node_descendant::NodeDescendant;
use charybdis::{CharybdisError, CharybdisModelBatch};
use scylla::CachingSession;

impl Node {
    pub async fn append_to_ancestors(
        &mut self,
        db_session: &CachingSession,
    ) -> Result<(), CharybdisError> {
        if let Some(ancestor_ids) = self.ancestor_ids.as_ref() {
            let mut batch = CharybdisModelBatch::new();

            for ancestor_id in ancestor_ids {
                let node_descendant = NodeDescendant {
                    root_id: self.root_id,
                    node_id: *ancestor_id,
                    id: self.id,
                    order_index: self.order_index.unwrap_or_default(),
                    parent_id: self.parent_id,
                    title: self.title.clone(),
                };

                batch.append_insert(&node_descendant)?;
            }

            batch.execute(db_session).await?;
        }

        Ok(())
    }

    pub async fn remove_from_ancestors(
        &mut self,
        db_session: &CachingSession,
    ) -> Result<(), CharybdisError> {
        if let Some(ancestor_ids) = self.ancestor_ids.as_ref() {
            let mut batch = CharybdisModelBatch::new();

            for ancestor_id in ancestor_ids {
                let node_descendant = NodeDescendant {
                    root_id: self.root_id,
                    node_id: ancestor_id.to_owned(),
                    id: self.id,
                    order_index: self.order_index.unwrap_or_default(),
                    ..Default::default()
                };

                batch.append_delete(&node_descendant)?;
            }

            batch.execute(db_session).await?;
        }

        Ok(())
    }
}

impl UpdateTitleNode {
    /// Updates the title of the node for node_descendant record within ancestor nodes.
    pub async fn update_node_descendants(
        &mut self,
        db_session: &CachingSession,
    ) -> Result<(), CharybdisError> {
        let mut batch = CharybdisModelBatch::new();

        if let Some(ancestor_ids) = self.ancestor_ids.as_ref() {
            for ancestor_id in ancestor_ids {
                let node_descendant = NodeDescendant {
                    root_id: self.root_id,
                    node_id: ancestor_id.to_owned(),
                    id: self.id,
                    parent_id: self.parent_id,
                    title: self.title.clone(),
                    order_index: self.order_index.unwrap_or_default(),
                };

                batch.append_update(&node_descendant)?;
            }
        }

        batch.execute(db_session).await?;

        Ok(())
    }
}
