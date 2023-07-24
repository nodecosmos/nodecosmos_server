use crate::models::node::Node;
use colored::Colorize;
use elasticsearch::http::response::Response;
use elasticsearch::indices::IndicesCreateParts;
use elasticsearch::Elasticsearch;
use serde_json::json;

pub async fn build_nodes_index(client: &Elasticsearch) {
    println!(
        "{} {}",
        "Building elastic index for".bright_green(),
        Node::ELASTIC_IDX_NAME.bright_yellow()
    );

    let response: Response = client
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
            "mappings": {
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
