use crate::errors::NodecosmosError;
use crate::models::node::Node;
use crate::models::node_descendant::NodeDescendant;
use charybdis::{CharybdisError, CharybdisModelBatch, CharybdisModelStream, Find};
use scylla::CachingSession;

impl Node {
    pub async fn parent(
        &self,
        db_session: &CachingSession,
    ) -> Result<Option<Node>, NodecosmosError> {
        if let Some(parent_id) = self.parent_id {
            let node = Self::find_by_primary_key_value(db_session, (parent_id,)).await?;

            Ok(Some(node))
        } else {
            Ok(None)
        }
    }

    pub async fn descendants(
        &self,
        db_session: &CachingSession,
    ) -> Result<CharybdisModelStream<NodeDescendant>, CharybdisError> {
        let descendants =
            NodeDescendant::find_by_root_id_and_node_id(db_session, self.root_id, self.id).await?;

        Ok(descendants)
    }

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
        descendants: &Vec<NodeDescendant>,
    ) -> Result<(), CharybdisError> {
        let mut descendants_to_delete = vec![];

        if let Some(ancestor_ids) = self.ancestor_ids.as_ref() {
            ancestor_ids.iter().for_each(|ancestor_id| {
                let node_descendant = NodeDescendant {
                    root_id: self.root_id,
                    node_id: *ancestor_id,
                    id: self.id,
                    order_index: self.order_index.unwrap_or_default(),
                    ..Default::default()
                };

                descendants_to_delete.push(node_descendant);

                // remove descendants from ancestors
                for descendant in descendants {
                    let node_descendant = NodeDescendant {
                        root_id: self.root_id,
                        node_id: *ancestor_id,
                        order_index: descendant.order_index,
                        id: descendant.id,
                        ..Default::default()
                    };

                    descendants_to_delete.push(node_descendant);
                }
            });
        }

        CharybdisModelBatch::chunked_delete(db_session, &descendants_to_delete, 100).await?;

        Ok(())
    }
}
