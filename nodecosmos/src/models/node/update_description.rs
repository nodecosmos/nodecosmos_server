use crate::models::node::{Node, UpdateDescriptionNode};
use crate::services::elastic::update_elastic_document;
use crate::CbExtension;
use ammonia::clean;

impl UpdateDescriptionNode {
    pub fn sanitize_description(&mut self) {
        if let Some(description) = &self.description {
            self.description = Some(clean(description));
        }
    }

    pub async fn update_elastic_index(&self, ext: &CbExtension) {
        update_elastic_document(&ext.elastic_client, Node::ELASTIC_IDX_NAME, self, self.id.to_string()).await;
    }
}
