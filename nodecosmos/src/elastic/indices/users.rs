use crate::models::user::User;
use colored::Colorize;
use elasticsearch::http::response::Response;
use elasticsearch::indices::IndicesCreateParts;
use elasticsearch::Elasticsearch;
use serde_json::json;

pub async fn build_user_index(client: &Elasticsearch) {
    println!(
        "{} {}",
        "Building elastic index for".bright_green(),
        User::ELASTIC_IDX_NAME.bright_yellow()
    );

    let response: Response = client
        .indices()
        .create(IndicesCreateParts::Index(User::ELASTIC_IDX_NAME))
        .body(json!({
            "settings": {
                "index": {
                    "number_of_shards": 2,
                    "number_of_replicas": 1
                }
            },
            "mappings": {
                "dynamic": false,
                "properties": {
                    "id": { "type": "keyword", "index": false },
                    "email": { "type": "search_as_you_type" },
                    "username": { "type": "search_as_you_type" },
                    "firstName": { "type": "search_as_you_type" },
                    "lastName": { "type": "search_as_you_type" },
                    "bio": { "type": "text" },
                    "createdAt": { "type": "date" },
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
        User::ELASTIC_IDX_NAME.bright_yellow()
    );
}
