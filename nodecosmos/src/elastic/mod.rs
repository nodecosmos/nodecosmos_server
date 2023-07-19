mod nodes;
mod users;

use elasticsearch::indices::IndicesExistsParts;
use elasticsearch::Elasticsearch;

pub async fn build(client: &Elasticsearch) {
    if !check_if_idx_exist(client, "nodes").await {
        nodes::build_nodes_index(client).await;
    }

    if !check_if_idx_exist(client, "users").await {
        users::build_user_index(client).await;
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
