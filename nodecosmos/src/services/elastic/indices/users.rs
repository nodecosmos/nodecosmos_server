use crate::models::user::User;
use crate::services::elastic::build::idx_exists;
use colored::Colorize;
use elasticsearch::indices::{IndicesCreateParts, IndicesPutMappingParts};
use elasticsearch::Elasticsearch;
use serde_json::json;

pub async fn build_user_index(client: &Elasticsearch) {
    let mappings = json!(
        {
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
    );

    let response;

    if idx_exists(client, User::ELASTIC_IDX_NAME).await {
        println!(
            "{} {}\n",
            "Sync elastic index for".bright_green(),
            User::ELASTIC_IDX_NAME.bright_yellow()
        );
        response = client
            .indices()
            .put_mapping(IndicesPutMappingParts::Index(&[User::ELASTIC_IDX_NAME]))
            .body(mappings)
            .send()
            .await;
    } else {
        println!(
            "{} {}\n",
            "Creating elastic index for".bright_green(),
            User::ELASTIC_IDX_NAME.bright_yellow()
        );
        response = client
            .indices()
            .create(IndicesCreateParts::Index(User::ELASTIC_IDX_NAME))
            .body(json!({
                "settings": {
                    "index": {
                        "number_of_shards": 2,
                        "number_of_replicas": 1
                    }
                },
                "mappings": mappings
            }))
            .send()
            .await
    }

    let response = response.unwrap_or_else(|e| {
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
    };
}
