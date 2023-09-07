use crate::elastic::indices;
use elasticsearch::indices::IndicesExistsParts;
use elasticsearch::Elasticsearch;

pub async fn build(client: &Elasticsearch) {
    indices::build_nodes_index(client).await;
    indices::build_user_index(client).await;
}

pub async fn idx_exists(client: &Elasticsearch, idx: &str) -> bool {
    let response = client
        .indices()
        .exists(IndicesExistsParts::Index(&[idx]))
        .send()
        .await
        .unwrap();
    response.status_code().is_success()
}
