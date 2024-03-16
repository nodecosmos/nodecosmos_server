use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::branch::update::BranchUpdate;
use crate::models::branch::Branch;
use crate::models::description_commit::DescriptionCommit;
use crate::models::node::UpdateDescriptionNode;
use crate::models::node_commit::create::NodeChange;
use crate::models::node_commit::NodeCommit;
use crate::models::traits::Branchable;
use crate::models::traits::ElasticDocument;
use charybdis::operations::Insert;
use elasticsearch::Elasticsearch;
use log::error;

impl UpdateDescriptionNode {
    pub async fn update_elastic_index(&self, elastic_client: &Elasticsearch) {
        if self.is_original() {
            self.update_elastic_document(elastic_client).await;
        }
    }

    pub async fn create_new_version(&self, data: &RequestData) {
        let description_version = DescriptionCommit::new(self.description_base64.clone());

        let _ = description_version
            .insert()
            .execute(data.db_session())
            .await
            .map_err(|e| {
                error!("[create_new_version::insert] {}", e);
                e
            });

        let changes = vec![NodeChange::Description(description_version.id)];

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
            error!("[create_new_version::handle_change] {}", e);
            e
        });
    }

    pub async fn update_branch(&self, data: &RequestData) -> Result<(), NodecosmosError> {
        if self.is_branched() {
            Branch::update(&data, self.branch_id, BranchUpdate::EditNodeDescription(self.id)).await?;
        }

        Ok(())
    }
}
