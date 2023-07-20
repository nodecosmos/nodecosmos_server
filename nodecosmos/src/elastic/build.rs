use crate::elastic::indices;
use crate::models::node::Node;
use crate::models::user::User;
use elasticsearch::indices::IndicesExistsParts;
use elasticsearch::Elasticsearch;

pub async fn build(client: &Elasticsearch) {
    if !check_if_idx_exist(client, Node::ELASTIC_IDX_NAME).await {
        indices::build_nodes_index(client).await;
    }

    if !check_if_idx_exist(client, User::ELASTIC_IDX_NAME).await {
        indices::build_user_index(client).await;
    }
}

pub async fn check_if_idx_exist(client: &Elasticsearch, idx: &str) -> bool {
    let response = client
        .indices()
        .exists(IndicesExistsParts::Index(&[idx]))
        .send()
        .await
        .unwrap();
    response.status_code().is_success()
}
