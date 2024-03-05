use crate::api::data::RequestData;
use crate::api::types::{ActionObject, ActionTypes};
use crate::models::branch::update::BranchUpdate;
use crate::models::branch::Branch;
use crate::models::node::{Node, UpdateTitleNode};
use crate::models::node_commit::create::NodeChange;
use crate::models::node_commit::NodeCommit;
use crate::models::node_descendant::NodeDescendant;
use crate::models::traits::Branchable;
use crate::models::traits::ElasticDocument;
use charybdis::batch::{CharybdisModelBatch, ModelBatch};
use charybdis::model::AsNative;
use elasticsearch::Elasticsearch;
use log::error;

impl UpdateTitleNode {
    /// Update self reference in node_descendants for each ancestor
    pub async fn update_title_for_ancestors(&self, data: &RequestData) {
        let mut native = self.as_native();

        if let Err(e) = native.transform_to_branched(data.db_session()).await {
            error!(
                "UpdateTitleNode::update_title_for_ancestors::transform_to_branched {}",
                e
            );
            return;
        }

        if let Err(e) = data.resource_locker().validate_node_root_unlocked(&native, true).await {
            error!(
                "UpdateTitleNode::update_title_for_ancestors::validate_node_root_unlocked: {}",
                e
            );
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
            error!(
                "UpdateTitleNode::update_title_for_ancestors::lock_resource_actions: {}",
                e
            );
            return;
        }

        if let Some(ancestor_ids) = native.ancestor_ids.clone() {
            let mut node_descendants = Vec::with_capacity(ancestor_ids.len());

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

                node_descendants.push(node_descendant);
            }

            if let Err(e) = NodeDescendant::batch()
                .chunked_update(data.db_session(), &node_descendants, 100)
                .await
            {
                error!("UpdateTitleNode::update_title_for_ancestors:chunked_update: {}", e);
            }
        }

        if let Err(e) = data
            .resource_locker()
            .unlock_resource_action(ActionTypes::Reorder(ActionObject::Node), &native.root_id.to_string())
            .await
        {
            error!(
                "UpdateTitleNode::update_title_for_ancestors::unlock_resource_action {}",
                e
            );
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
            error!("UpdateTitleNode::create_new_version {}", e);
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
