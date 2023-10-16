use crate::actions::types::{ActionObject, ActionTypes};
use crate::errors::NodecosmosError;
use crate::models::node::{partial_node, Node};
use crate::models::node_descendant::NodeDescendant;
use crate::services::elastic::update_elastic_document;
use crate::services::logger::log_error;
use crate::CbExtension;
use charybdis::batch::CharybdisModelBatch;
use charybdis::callbacks::ExtCallbacks;
use charybdis::model::AsNative;
use charybdis::operations::Find;
use charybdis::types::{Text, Timestamp, Uuid};
use scylla::CachingSession;
use serde::{Deserialize, Serialize};

partial_node!(UpdateTitleNode, root_id, id, title, updated_at);

impl UpdateTitleNode {
    /// Update self reference in node_descendants for each ancestor
    pub async fn update_title_for_ancestors(
        &mut self,
        db_session: &CachingSession,
        ext: &CbExtension,
    ) -> Result<(), NodecosmosError> {
        // Here, we lock reorder action for current actions as in Scylla, updates can result in
        // insertion of new records if they don't exist.
        // This would be problematic as if ancestor changes in the middle of update, it would result
        // in wrong order_index and wrong title for node_descendant record.
        ext.resource_locker
            .lock_resource_action(
                ActionTypes::Reorder(ActionObject::Node),
                &self.root_id.to_string(),
                1000,
            )
            .await?;

        // find native node after locking reorder action
        let native = self.as_native().find_by_primary_key(db_session).await?;

        if let Some(ancestor_ids) = native.ancestor_ids {
            let mut batch = CharybdisModelBatch::new();

            for ancestor_id in ancestor_ids {
                let node_descendant = NodeDescendant {
                    root_id: self.root_id,
                    node_id: ancestor_id,
                    id: self.id,
                    parent_id: native.parent_id.unwrap(), // we must have parent_id as we have ancestor_ids
                    title: self.title.clone(),
                    order_index: native.order_index,
                };

                batch.append_update(&node_descendant)?;
            }

            let res = batch.execute(db_session).await;

            match res {
                Ok(_) => {
                    ext.resource_locker
                        .unlock_resource_action(ActionTypes::Reorder(ActionObject::Node), &self.root_id.to_string())
                        .await?;
                }
                Err(e) => {
                    log_error(format!(
                        "Error updating title for node descendants: {}. Node reorder will remain locked",
                        e
                    ));
                }
            }
        }

        ext.resource_locker
            .unlock_resource_action(ActionTypes::Reorder(ActionObject::Node), &self.root_id.to_string())
            .await?;

        Ok(())
    }

    pub async fn update_elastic_index(&self, ext: &CbExtension) {
        update_elastic_document(&ext.elastic_client, Node::ELASTIC_IDX_NAME, self, self.id.to_string()).await;
    }
}

impl ExtCallbacks<CbExtension, NodecosmosError> for UpdateTitleNode {
    async fn before_update(&mut self, session: &CachingSession, ext: &CbExtension) -> Result<(), NodecosmosError> {
        self.update_title_for_ancestors(session, ext).await?;
        self.update_elastic_index(ext).await;
        self.updated_at = Some(chrono::Utc::now());

        Ok(())
    }
}
