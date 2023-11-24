use crate::models::node::Node;
use crate::services::elastic::index::ElasticIndex;
use serde_json::{json, Value};

impl ElasticIndex for Node {
    const ELASTIC_IDX_NAME: &'static str = "nodes";

    fn settings_json() -> Value {
        json!({
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
        })
    }

    fn mappings_json() -> Value {
        json!(
            {
                "dynamic": false,
                "properties": {
                    "ancestorIds": {
                        "type": "keyword",
                        "index": false
                    },
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
                    "likesCount": { "type": "integer" }
                }
            }
        )
    }
}
