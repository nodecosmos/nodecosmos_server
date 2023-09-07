use crate::models::node::Node;
use crate::services::elastic::build::idx_exists;
use colored::Colorize;
use elasticsearch::indices::{IndicesCreateParts, IndicesPutMappingParts};
use elasticsearch::Elasticsearch;
use serde_json::json;

pub async fn build_nodes_index(client: &Elasticsearch) {
    let mappings = json!(
        {
            "dynamic": false,
            "properties": {
                "rootId": { "type": "keyword", "index": false },
                "id": { "type": "keyword", "index": false },
                "title": { "type": "text", "analyzer": "english" },
                "shortDescription": { "type": "text", "index": false  },
                "description": {
                    "type": "text",
                    "analyzer": "english_with_html_strip",
                },
                "isRoot": { "type": "boolean" },
                "isPublic": { "type": "boolean" },
                "createdAt": { "type": "date" },
                "likesCount": { "type": "integer" },
                "coverImage": { "type": "keyword", "index": false },
            }
        }
    );

    let response;

    if idx_exists(client, Node::ELASTIC_IDX_NAME).await {
        println!(
            "\n{} {}",
            "Updating elastic index for".bright_green(),
            Node::ELASTIC_IDX_NAME.bright_yellow()
        );

        response = client
            .indices()
            .put_mapping(IndicesPutMappingParts::Index(&[Node::ELASTIC_IDX_NAME]))
            .body(mappings)
            .send()
            .await;
    } else {
        println!(
            "\n{} {}",
            "Creating elastic index for".bright_green(),
            Node::ELASTIC_IDX_NAME.bright_yellow()
        );
        response = client
            .indices()
            .create(IndicesCreateParts::Index(Node::ELASTIC_IDX_NAME))
            .body(json!({
                "settings": {
                    "analysis": {
                      "analyzer": {
                        "english_with_html_strip": {
                          "tokenizer": "standard",
                          "char_filter": ["html_strip"],
                          "filter": [
                            "english_possessive_stemmer",
                            "lowercase",
                            "english_stop",
                            "english_stemmer"
                          ]
                        }
                      },
                      "filter": {
                        "english_possessive_stemmer": {
                          "type": "stemmer",
                          "language": "possessive_english"
                        },
                        "english_stop": {
                          "type": "stop",
                          "stopwords": "_english_"
                        },
                        "english_stemmer": {
                          "type": "stemmer",
                          "language": "english"
                        }
                      }
                    },
                    "index": {
                        "number_of_shards": 2,
                        "number_of_replicas": 1
                    }
                },
                "mappings": mappings
            }))
            .send()
            .await;
    }

    let response = response.unwrap_or_else(|e| {
        panic!("Failed to handle node index: {:#?}", e);
    });

    if !response.status_code().is_success() {
        panic!(
            "Failed to handle node index: {}! Response body: {}",
            response.status_code(),
            response
                .text()
                .await
                .unwrap_or("No Body!".to_string())
                .bright_red()
        );
    }
}
