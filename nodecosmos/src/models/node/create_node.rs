use crate::app::CbExtension;
use crate::models::node::Node;
use crate::services::elastic::add_elastic_document;

impl Node {
    pub async fn add_to_elastic_index(&self, ext: &CbExtension) {
        add_elastic_document(
            &ext.elastic_client,
            Node::ELASTIC_IDX_NAME,
            self,
            self.id.to_string(),
        )
        .await;
    }
}
