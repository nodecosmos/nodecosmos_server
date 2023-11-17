use crate::api::data::RequestData;
use crate::models::node::{Node, UpdateDescriptionNode};
use crate::models::versioned_description::VersionedDescription;
use crate::models::versioned_node::create::NodeChange;
use crate::models::versioned_node::VersionedNode;
use crate::services::elastic::index::ElasticIndex;
use crate::services::elastic::update_elastic_document;
use crate::utils::logger::log_error;
use crate::App;
use ammonia::clean;
use charybdis::model::AsNative;
use charybdis::operations::Insert;

impl UpdateDescriptionNode {
    pub fn sanitize_description(&mut self) {
        if let Some(description) = &self.description {
            self.description = Some(clean(description));
        }
    }

    pub async fn update_elastic_index(&self, app: &App) {
        update_elastic_document(&app.elastic_client, Node::ELASTIC_IDX_NAME, self, self.id.to_string()).await;
    }

    pub async fn create_new_version(&self, ext: &RequestData) {
        let native = self.as_native();
        let session = &ext.app.db_session;
        let description_version = VersionedDescription::new(
            self.description.clone(),
            self.short_description.clone(),
            self.description_markdown.clone(),
            self.description_base64.clone(),
        );

        let change = NodeChange::Description(description_version.id);
        let _ = description_version.insert(session).await;

        let _ = VersionedNode::handle_change(session, &native, vec![change], ext.current_user.id)
            .await
            .map_err(|e| {
                log_error(format!("Failed to create new versioned node: {}", e));
            });
    }
}
