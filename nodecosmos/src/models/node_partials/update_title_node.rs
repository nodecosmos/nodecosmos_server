use crate::actions::types::{ActionObject, ActionTypes};
use crate::errors::NodecosmosError;
use crate::models::helpers::ClonedRef;
use crate::models::node::{partial_node, Node};
use crate::models::node_descendant::NodeDescendant;
use crate::services::elastic::update_elastic_document;
use crate::services::logger::log_error;
use crate::CbExtension;
use charybdis::batch::CharybdisModelBatch;
use charybdis::callbacks::ExtCallbacks;
use charybdis::types::{Double, Set, Text, Timestamp, Uuid};
use chrono::Utc;
use scylla::CachingSession;
use serde::{Deserialize, Serialize};

partial_node!(
    UpdateTitleNode,
    root_id,
    parent_id,
    id,
    title,
    ancestor_ids,
    order_index,
    updated_at
);

impl UpdateTitleNode {
    pub fn set_defaults(&mut self, native_node: Node) {
        self.updated_at = Some(Utc::now());

        self.root_id = native_node.root_id;
        self.parent_id = native_node.parent_id;
        self.order_index = native_node.order_index;
        self.ancestor_ids = native_node.ancestor_ids;
    }

    /// Update self reference in node_descendants for each ancestor
    pub async fn update_ancestors(
        &mut self,
        db_session: &CachingSession,
        ext: &CbExtension,
    ) -> Result<(), NodecosmosError> {
        let mut batch = CharybdisModelBatch::new();

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

        for ancestor_id in self.ancestor_ids.cloned_ref() {
            let node_descendant = NodeDescendant {
                root_id: self.root_id,
                node_id: ancestor_id,
                id: self.id,
                parent_id: self.parent_id.unwrap(),
                title: self.title.clone().unwrap(),
                order_index: self.order_index.unwrap_or_default(),
            };

            batch.append_update(&node_descendant)?;
        }

        let res = batch.execute(db_session).await;

        match res {
            Ok(_) => {
                ext.resource_locker
                    .unlock_resource_action(
                        ActionTypes::Reorder(ActionObject::Node),
                        &self.root_id.to_string(),
                    )
                    .await?;
            }
            Err(e) => {
                log_error(format!("Error updating title for node descendants: {}. Node reorder will remain locked", e));
            }
        }

        ext.resource_locker
            .unlock_resource_action(
                ActionTypes::Reorder(ActionObject::Node),
                &self.root_id.to_string(),
            )
            .await?;

        Ok(())
    }

    pub async fn update_elastic_index(&self, ext: &CbExtension) {
        update_elastic_document(
            &ext.elastic_client,
            Node::ELASTIC_IDX_NAME,
            self,
            self.id.to_string(),
        )
        .await;
    }
}

impl ExtCallbacks<CbExtension, NodecosmosError> for UpdateTitleNode {
    async fn before_update(
        &mut self,
        session: &CachingSession,
        ext: &CbExtension,
    ) -> Result<(), NodecosmosError> {
        self.update_ancestors(session, ext).await?;
        self.update_elastic_index(ext).await;

        Ok(())
    }
}
