use crate::models::node::{Node, UpdateNodeTitle};
use crate::models::node_descendant::{NodeDescendant, UpdateNodeDescendantTitle};
use charybdis::{CharybdisError, CharybdisModelBatch};
use scylla::CachingSession;

impl Node {
    pub async fn append_to_ancestors(
        &mut self,
        db_session: &CachingSession,
    ) -> Result<(), CharybdisError> {
        let mut batch = CharybdisModelBatch::new();

        if let Some(ancestor_ids) = self.ancestor_ids.as_ref() {
            let ancestor_chunks = ancestor_ids.chunks(100);

            for ancestor_ids in ancestor_chunks {
                for ancestor_id in ancestor_ids {
                    let node_descendant = NodeDescendant {
                        root_id: ancestor_id.to_owned(),
                        id: self.id,
                        order_index: self.order_index,
                        parent_id: self.parent_id,
                        title: self.title.clone(),
                    };

                    batch.append_create(&node_descendant)?;
                }
            }
        }

        batch.execute(db_session).await?;

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
                    root_id: ancestor_id.to_owned(),
                    id: self.id,
                    order_index: self.order_index,
                    ..Default::default()
                };

                batch.append_delete(&node_descendant)?;
            }

            batch.execute(db_session).await?;
        }

        Ok(())
    }
}

impl UpdateNodeTitle {
    /// Updates the title of the node for all ancestors.
    pub async fn update_ancestors(
        &mut self,
        db_session: &CachingSession,
    ) -> Result<(), CharybdisError> {
        let mut batch = CharybdisModelBatch::new();

        if let Some(ancestor_ids) = self.ancestor_ids.as_ref() {
            println!("ancestor_ids: {:?}", ancestor_ids);
            for ancestor_id in ancestor_ids {
                let node_descendant = UpdateNodeDescendantTitle {
                    root_id: ancestor_id.to_owned(),
                    id: self.id,
                    title: self.title.clone(),
                    order_index: self.order_index,
                };

                batch.append_update(&node_descendant)?;
            }
        }

        batch.execute(db_session).await?;

        Ok(())
    }
}
