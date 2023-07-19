use colored::Colorize;
use elasticsearch::http::response::Response;
use elasticsearch::indices::IndicesCreateParts;
use elasticsearch::Elasticsearch;
use serde_json::json;

pub async fn build_user_index(client: &Elasticsearch) {
    println!(
        "{} {}",
        "Building elastic index for".bright_green(),
        "users".bright_yellow()
    );

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
                    "id": { "type": "keyword", "index": false },
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
        "{} {}\n",
        "Successfully created elastic index".bright_green(),
        "users".bright_yellow()
    );
}
