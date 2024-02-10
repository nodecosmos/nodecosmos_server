use crate::api::data::RequestData;
use crate::api::types::{ActionObject, ActionTypes};
use crate::models::branch::update::BranchUpdate;
use crate::models::branch::Branch;
use crate::models::node::{Node, UpdateTitleNode};
use crate::models::node_commit::create::NodeChange;
use crate::models::node_commit::NodeCommit;
use crate::models::node_descendant::NodeDescendant;
use crate::models::traits::Branchable;
use crate::services::elastic::ElasticDocument;
use crate::utils::logger::{log_error, log_fatal};
use charybdis::batch::CharybdisModelBatch;
use charybdis::model::AsNative;
use elasticsearch::Elasticsearch;

impl UpdateTitleNode {
    /// Update self reference in node_descendants for each ancestor
    pub async fn update_title_for_ancestors(&self, data: &RequestData) {
        let mut native = self.as_native();

        if let Err(e) = native.transform_to_branched(data.db_session()).await {
            log_error(format!("Failed to transform node to branched: {}", e));
            return;
        }

        if let Err(e) = data.resource_locker().validate_node_unlocked(&native, true).await {
            log_fatal(format!("Failed to validate node unlocked for title update: {}", e));
            return;
        }

        if let Err(e) = data
            .resource_locker()
            .lock_resource_actions(
                &native.root_id.to_string(),
                vec![ActionTypes::Reorder(ActionObject::Node), ActionTypes::Merge],
                1000,
            )
            .await
        {
            log_fatal(format!("Failed to lock resource for title update: {}", e));
            return;
        }

        if let Some(ancestor_ids) = native.ancestor_ids.clone() {
            let mut batch = CharybdisModelBatch::new();

            for ancestor_id in ancestor_ids {
                let node_descendant = NodeDescendant {
                    root_id: native.root_id,
                    branch_id: native.branchise_id(ancestor_id),
                    node_id: ancestor_id,
                    id: self.id,
                    parent_id: native.parent_id.unwrap(), // we must have parent_id as we have ancestor_ids
                    title: self.title.clone(),
                    order_index: native.order_index,
                };

                if let Err(e) = batch.append_update(node_descendant) {
                    log_error(format!("Failed to append update node_descendants: {}", e));
                }
            }

            if let Err(e) = batch.execute(data.db_session()).await {
                log_error(format!("Failed to update node_descendants: {}", e));
            }
        }

        if let Err(e) = data
            .resource_locker()
            .unlock_resource_action(ActionTypes::Reorder(ActionObject::Node), &native.root_id.to_string())
            .await
        {
            log_error(format!("Failed to unlock resource: {}", e));
        }
    }

    pub async fn update_elastic_index(&self, elastic_client: &Elasticsearch) {
        if self.is_original() {
            self.update_elastic_document(elastic_client).await;
        }
    }

    pub async fn create_new_version(&self, data: &RequestData) {
        let changes = vec![NodeChange::Title(self.title.clone())];

        let _ = NodeCommit::handle_change(
            data.db_session(),
            self.id,
            self.branch_id,
            data.current_user_id(),
            &changes,
            true,
        )
        .await
        .map_err(|e| {
            log_error(format!("Failed to create new versioned node: {}", e));
        });
    }
    pub async fn update_branch(&self, req_data: &RequestData) {
        if self.is_branched() {
            Branch::update(
                &req_data.db_session(),
                self.branch_id,
                BranchUpdate::EditNodeTitle(self.id),
            )
            .await;
        }
    }
}
