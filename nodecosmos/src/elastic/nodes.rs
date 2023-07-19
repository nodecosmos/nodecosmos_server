use colored::Colorize;
use elasticsearch::http::response::Response;
use elasticsearch::indices::IndicesCreateParts;
use elasticsearch::Elasticsearch;
use serde_json::json;

pub async fn build_nodes_index(client: &Elasticsearch) {
    println!(
        "{} {}",
        "Building elastic index for".bright_green(),
        "nodes".bright_yellow()
    );

    let response: Response = client
        .indices()
        .create(IndicesCreateParts::Index("nodes"))
        .body(json!({
            "settings": {
                "index": {
                    "number_of_shards": 2,
                    "number_of_replicas": 1
                }
            },
            "mappings": {
                "properties": {
                    "root_id": { "type": "keyword", "index": false },
                    "id": { "type": "keyword", "index": false },
                    "title": { "type": "text" },
                    "description": { "type": "text" },
                    "is_root": { "type": "boolean" },
                    "is_public": { "type": "boolean" },
                    "created_at": { "type": "date" },
                    "likes_count": { "type": "integer" },
                }
            }
        }))
        .send()
        .await
        .unwrap_or_else(|e| {
            panic!("Failed to create node index: {}", e);
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
        "nodes".bright_yellow()
    );
}
