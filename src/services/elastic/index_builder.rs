use crate::models::node::Node;
use crate::models::user::User;
use crate::services::elastic::index::BuildIndex;
use elasticsearch::Elasticsearch;

pub struct ElasticIndexBuilder<'a> {
    client: &'a Elasticsearch,
}

impl<'a> ElasticIndexBuilder<'a> {
    pub fn new(client: &'a Elasticsearch) -> Self {
        Self { client }
    }

    pub async fn build(&self) {
        Node::build_index(&self.client).await;
        User::build_index(&self.client).await;
    }
}
