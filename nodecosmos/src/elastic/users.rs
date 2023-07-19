use colored::Colorize;
use elasticsearch::http::response::Response;
use elasticsearch::indices::IndicesCreateParts;
use elasticsearch::Elasticsearch;
use serde_json::json;

pub async fn build_user_index(client: &Elasticsearch) {
    let response: Response = client
        .indices()
        .create(IndicesCreateParts::Index("users"))
        .body(json!({
            "settings": {
                "index": {
                    "number_of_shards": 2,
                    "number_of_replicas": 1
                }
            },
            "mappings": {
                "properties": {
                    "id": { "type": "keyword" },
                    "email": { "type": "keyword" },
                    "username": { "type": "keyword" },
                    "first_name": { "type": "text" },
                    "last_name": { "type": "text" },
                    "created_at": { "type": "date" },
                }
            }
        }))
        .send()
        .await
        .unwrap_or_else(|e| {
            panic!("Failed to create users index: {}", e);
        });

    if !response.status_code().is_success() {
        panic!(
            "Failed to create node index: {}! Response body: {}",
            response.status_code(),
            response
                .text()
                .await
                .unwrap_or("No Body!".to_string())
                .bright_red()
        );
    }

    println!(
        "{}",
        "Index for 'users' created successfully!".bright_green()
    );
}
