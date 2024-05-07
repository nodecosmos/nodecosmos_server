use charybdis::batch::ModelBatch;
use elasticsearch::Elasticsearch;
use log::error;

use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::branch::update::BranchUpdate;
use crate::models::branch::Branch;
use crate::models::node::UpdateTitleNode;
use crate::models::node_descendant::NodeDescendant;
use crate::models::traits::Branchable;
use crate::models::traits::ElasticDocument;

impl UpdateTitleNode {
    /// Update self reference in node_descendants for each ancestor
    pub async fn update_title_for_ancestors(&self, data: &RequestData) -> Result<(), NodecosmosError> {
        if let Some(ancestor_ids) = &self.ancestor_ids {
            let mut node_descendants = Vec::with_capacity(ancestor_ids.len());

            for ancestor_id in ancestor_ids {
                let node_descendant = NodeDescendant {
                    root_id: self.root_id,
                    branch_id: self.branch_id,
                    node_id: *ancestor_id,
                    id: self.id,
                    parent_id: self.parent_id.expect("Parent id is should be set"),
                    title: self.title.clone(),
                    order_index: self.order_index,
                };

                node_descendants.push(node_descendant);
            }

            if let Err(e) = NodeDescendant::batch()
                .chunked_update(data.db_session(), &node_descendants, crate::constants::BATCH_CHUNK_SIZE)
                .await
            {
                error!(":chunked_update: {}", e);
            }
        }

        Ok(())
    }

    pub async fn update_branch(&self, data: &RequestData) -> Result<(), NodecosmosError> {
        if self.is_branch() {
            Branch::update(data.db_session(), self.branch_id, BranchUpdate::EditNodeTitle(self.id)).await?;
        }

        Ok(())
    }

    pub async fn update_elastic_index(&self, elastic_client: &Elasticsearch) {
        if self.is_original() {
            self.update_elastic_document(elastic_client).await;
        }
    }
}
