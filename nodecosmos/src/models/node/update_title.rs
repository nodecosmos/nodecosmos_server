use crate::api::data::RequestData;
use crate::api::types::{ActionObject, ActionTypes};
use crate::errors::NodecosmosError;
use crate::models::node::{Node, UpdateTitleNode};
use crate::models::node_descendant::NodeDescendant;
use crate::models::versioned_node::create::NodeChange;
use crate::models::versioned_node::VersionedNode;
use crate::services::elastic::index::ElasticIndex;
use crate::services::elastic::update_elastic_document;
use crate::utils::logger::log_error;
use crate::App;
use charybdis::batch::CharybdisModelBatch;
use charybdis::model::AsNative;
use charybdis::operations::Find;
use scylla::CachingSession;

impl UpdateTitleNode {
    /// Update self reference in node_descendants for each ancestor
    pub async fn update_title_for_ancestors(
        &mut self,
        db_session: &CachingSession,
        app: &App,
    ) -> Result<(), NodecosmosError> {
        // Here, we lock reorder action for current actions as updates can result in
        // insertion of new records if they don't exist.
        // This would be problematic as if ancestor changes in the middle of update, it would result
        // in wrong order_index and wrong title for node_descendant record.
        app.resource_locker
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
                    app.resource_locker
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

        app.resource_locker
            .unlock_resource_action(ActionTypes::Reorder(ActionObject::Node), &self.root_id.to_string())
            .await?;

        Ok(())
    }

    pub async fn update_elastic_index(&self, app: &App) {
        update_elastic_document(&app.elastic_client, Node::ELASTIC_IDX_NAME, self, self.id.to_string()).await;
    }

    pub async fn create_new_version(&self, ext: &RequestData) {
        let native = self.as_native();
        let session = &ext.app.db_session;

        let change = NodeChange::Title(self.title.clone());

        let _ = VersionedNode::handle_change(session, &native, vec![change], ext.current_user.id)
            .await
            .map_err(|e| {
                log_error(format!("Failed to create new versioned node: {}", e));
            });
    }
}
