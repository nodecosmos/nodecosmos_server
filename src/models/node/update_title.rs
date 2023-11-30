use crate::api::data::RequestData;
use crate::api::types::{ActionObject, ActionTypes};
use crate::errors::NodecosmosError;
use crate::models::branch::branchable::Branchable;
use crate::models::node::{Node, UpdateTitleNode};
use crate::models::node_commit::create::NodeChange;
use crate::models::node_commit::NodeCommit;
use crate::models::node_descendant::NodeDescendant;
use crate::services::elastic::index::ElasticIndex;
use crate::services::elastic::update_elastic_document;
use crate::utils::logger::log_error;
use charybdis::batch::CharybdisModelBatch;
use charybdis::model::AsNative;
use charybdis::operations::Find;
use charybdis::types::Uuid;
use elasticsearch::Elasticsearch;

impl UpdateTitleNode {
    pub async fn validate_root(&self) -> Result<(), NodecosmosError> {
        if self.root_id != self.id {
            if self.root_id == Uuid::default() {
                return Err(NodecosmosError::ValidationError((
                    "root_id".to_string(),
                    "is invalid".to_string(),
                )));
            }
        }

        Ok(())
    }

    /// Update self reference in node_descendants for each ancestor
    pub async fn update_title_for_ancestors(&mut self, req_data: &RequestData) -> Result<(), NodecosmosError> {
        // Here, we lock reorder action for current actions as updates can result in
        // insertion of new records if they don't exist.
        // This would be problematic as if ancestor changes in the middle of update, it would result
        // in wrong order_index and wrong title for node_descendant record.
        req_data
            .resource_locker()
            .lock_resource_action(
                ActionTypes::Reorder(ActionObject::Node),
                &self.root_id.to_string(),
                1000,
            )
            .await?;

        // find native node after locking reorder action
        let native = self.as_native().find_by_primary_key(req_data.db_session()).await?;

        if let Some(ancestor_ids) = native.ancestor_ids.clone() {
            let mut batch = CharybdisModelBatch::new();

            for ancestor_id in ancestor_ids {
                let node_descendant = NodeDescendant {
                    root_id: self.root_id,
                    branch_id: native.branched_id(ancestor_id),
                    node_id: ancestor_id,
                    id: self.id,
                    parent_id: native.parent_id.unwrap(), // we must have parent_id as we have ancestor_ids
                    title: self.title.clone(),
                    order_index: native.order_index,
                };

                batch.append_update(&node_descendant)?;
            }

            let res = batch.execute(req_data.db_session()).await;

            match res {
                Ok(_) => {
                    req_data
                        .resource_locker()
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

        req_data
            .resource_locker()
            .unlock_resource_action(ActionTypes::Reorder(ActionObject::Node), &self.root_id.to_string())
            .await?;

        Ok(())
    }

    pub async fn update_elastic_index(&self, elastic_client: &Elasticsearch) {
        update_elastic_document(elastic_client, Node::ELASTIC_IDX_NAME, self, self.id.to_string()).await;
    }

    pub async fn create_new_version(&self, req_data: &RequestData) {
        let changes = vec![NodeChange::Title(self.title.clone())];

        let _ = NodeCommit::handle_change(
            req_data.db_session(),
            self.id,
            self.branch_id,
            req_data.current_user_id(),
            &changes,
            true,
        )
        .await
        .map_err(|e| {
            log_error(format!("Failed to create new versioned node: {}", e));
        });
    }
}
