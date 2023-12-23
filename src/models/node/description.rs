use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::branch::branchable::Branchable;
use crate::models::branch::update::BranchUpdate;
use crate::models::branch::Branch;
use crate::models::description_commit::DescriptionCommit;
use crate::models::node::{GetDescriptionBase64Node, GetDescriptionNode, Node, UpdateDescriptionNode};
use crate::models::node_commit::create::NodeChange;
use crate::models::node_commit::NodeCommit;
use crate::services::elastic::index::ElasticIndex;
use crate::services::elastic::update_elastic_document;
use crate::utils::logger::log_error;
use ammonia::clean;
use charybdis::operations::{Find, Insert};
use elasticsearch::Elasticsearch;
use scylla::CachingSession;

impl UpdateDescriptionNode {
    pub fn sanitize_description(&mut self) {
        if let Some(description) = &self.description {
            self.description = Some(clean(description));
        }
    }

    pub async fn update_elastic_index(&self, elastic_client: &Elasticsearch) {
        if self.is_main_branch() {
            update_elastic_document(elastic_client, Node::ELASTIC_IDX_NAME, self, self.id.to_string()).await;
        }
    }

    pub async fn create_new_version(&self, req_data: &RequestData) {
        let description_version = DescriptionCommit::new(self.description_base64.clone());

        let _ = description_version.insert(req_data.db_session()).await.map_err(|e| {
            log_error(format!("Failed to create new description version: {}", e));
            e
        });

        let changes = vec![NodeChange::Description(description_version.id)];

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
            log_error(format!("Failed to create new node version: {}", e));
            e
        });
    }

    pub async fn update_branch(&self, req_data: &RequestData) {
        if self.is_different_branch() {
            Branch::update(
                &req_data.db_session(),
                self.branch_id,
                BranchUpdate::EditNodeDescription(self.id),
            )
            .await;
        }
    }
}

impl GetDescriptionNode {
    pub async fn find_branched(&mut self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        let branch_self = Self::find_by_primary_key_value(db_session, (self.id, self.branch_id))
            .await
            .ok();

        match branch_self {
            Some(branch_self) => {
                *self = branch_self;
            }
            None => {
                let branch_id = self.branch_id;

                *self = Self::find_by_primary_key_value(db_session, (self.id, self.id)).await?;
                self.branch_id = branch_id;
            }
        }

        Ok(())
    }
}

impl GetDescriptionBase64Node {
    pub async fn find_branched(&mut self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        let branch_self = Self::find_by_primary_key_value(db_session, (self.id, self.branch_id))
            .await
            .ok();

        match branch_self {
            Some(branch_self) => {
                *self = branch_self;
            }
            None => {
                let branch_id = self.branch_id;

                *self = Self::find_by_primary_key_value(db_session, (self.id, self.id)).await?;
                self.branch_id = branch_id;
            }
        }

        Ok(())
    }
}
