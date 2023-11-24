use elasticsearch::indices::IndicesExistsParts;
use elasticsearch::Elasticsearch;

pub async fn idx_exists(client: &Elasticsearch, idx: &str) -> bool {
    let response = client
        .indices()
        .exists(IndicesExistsParts::Index(&[idx]))
        .send()
        .await
        .unwrap();

    let status = response.status_code().clone();

    status.is_success()
}
