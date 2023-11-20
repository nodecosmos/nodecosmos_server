use crate::api::data::RequestData;
use crate::models::node::{Node, UpdateDescriptionNode};
use crate::models::versioned_description::VersionedDescription;
use crate::models::versioned_node::create::NodeChange;
use crate::models::versioned_node::VersionedNode;
use crate::services::elastic::index::ElasticIndex;
use crate::services::elastic::update_elastic_document;
use crate::utils::logger::log_error;
use ammonia::clean;
use charybdis::operations::Insert;
use elasticsearch::Elasticsearch;

impl UpdateDescriptionNode {
    pub fn sanitize_description(&mut self) {
        if let Some(description) = &self.description {
            self.description = Some(clean(description));
        }
    }

    pub async fn update_elastic_index(&self, elastic_client: &Elasticsearch) {
        update_elastic_document(elastic_client, Node::ELASTIC_IDX_NAME, self, self.id.to_string()).await;
    }

    pub async fn create_new_version(&self, req_data: &RequestData) {
        let description_version = VersionedDescription::new(
            self.description.clone(),
            self.short_description.clone(),
            self.description_markdown.clone(),
            self.description_base64.clone(),
        );

        let _ = description_version.insert(req_data.db_session()).await.map_err(|e| {
            log_error(format!("Failed to create new description version: {}", e));
            e
        });

        let changes = vec![NodeChange::Description(description_version.id)];

        let _ = VersionedNode::handle_change(
            req_data.db_session(),
            self.id,
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
}
