use crate::models::node::{
    Node, UpdateCoverImageNode, UpdateDescriptionNode, UpdateLikesCountNode, UpdateOwnerNode, UpdateTitleNode,
};
use crate::models::user::{UpdateProfileImageUser, UpdateUser, User};
use crate::services::elastic::utils::idx_exists;
use charybdis::model::{AsNative, BaseModel, Model};
use charybdis::types::Uuid;
use colored::Colorize;
use elasticsearch::indices::{IndicesCreateParts, IndicesPutMappingParts};
use elasticsearch::Elasticsearch;
use serde::Serialize;
use serde_json::{json, Value};

pub struct ElasticIndexBuilder<'a> {
    client: &'a Elasticsearch,
}

impl<'a> ElasticIndexBuilder<'a> {
    pub fn new(client: &'a Elasticsearch) -> Self {
        Self { client }
    }

    pub async fn build(&self) {
        Node::build_index(&self.client).await;
        User::build_index(&self.client).await;
    }
}

pub trait ElasticIndex {
    const ELASTIC_IDX_NAME: &'static str;

    fn settings_json() -> Value;
    fn mappings_json() -> Value;

    fn index_id(&self) -> String;
}

pub trait BuildIndex: ElasticIndex {
    async fn build_index(client: &Elasticsearch);
}

impl<T: ElasticIndex> BuildIndex for T {
    async fn build_index(client: &Elasticsearch) {
        let response;

        if idx_exists(client, Self::ELASTIC_IDX_NAME).await {
            println!(
                "{} {}",
                "Sync elastic index for".bright_green(),
                Self::ELASTIC_IDX_NAME.bright_yellow()
            );

            response = client
                .indices()
                .put_mapping(IndicesPutMappingParts::Index(&[Self::ELASTIC_IDX_NAME]))
                .body(Self::mappings_json())
                .send()
                .await;
        } else {
            println!(
                "{} {}",
                "Creating elastic index for".bright_green(),
                Self::ELASTIC_IDX_NAME.bright_yellow()
            );
            response = client
                .indices()
                .create(IndicesCreateParts::Index(Self::ELASTIC_IDX_NAME))
                .body(json!({
                    "settings": Self::mappings_json(),
                    "mappings": Self::settings_json()
                }))
                .send()
                .await;
        }

        let response = response.unwrap_or_else(|e| {
            panic!("Failed to handle node index: {:#?}", e);
        });

        if !response.status_code().is_success() {
            eprintln!("Failed Elasticsearch operation. Debug info: ...");

            panic!(
                "Failed to handle node index: {}! Response body: {}",
                response.status_code(),
                response.text().await.unwrap_or("No Body!".to_string()).bright_red()
            );
        }
    }
}

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

    fn index_id(&self) -> String {
        self.id.to_string()
    }
}

impl ElasticIndex for User {
    const ELASTIC_IDX_NAME: &'static str = "users";

    fn settings_json() -> Value {
        json!({
            "settings": {
                "index": {
                    "number_of_shards": 2,
                    "number_of_replicas": 1
                }
            }
        })
    }

    fn mappings_json() -> Value {
        json!(
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
        )
    }

    fn index_id(&self) -> String {
        self.id.to_string()
    }
}

macro_rules! impl_elastic_index {
    ($target_struct:ident, $source_struct:ident) => {
        impl ElasticIndex for $target_struct {
            const ELASTIC_IDX_NAME: &'static str = $source_struct::ELASTIC_IDX_NAME;

            fn settings_json() -> Value {
                $source_struct::settings_json()
            }

            fn mappings_json() -> Value {
                $source_struct::mappings_json()
            }

            fn index_id(&self) -> String {
                self.id.to_string()
            }
        }
    };
}

impl_elastic_index!(UpdateTitleNode, Node);
impl_elastic_index!(UpdateDescriptionNode, Node);
impl_elastic_index!(UpdateCoverImageNode, Node);
impl_elastic_index!(UpdateLikesCountNode, Node);
impl_elastic_index!(UpdateOwnerNode, Node);

impl_elastic_index!(UpdateUser, User);
impl_elastic_index!(UpdateProfileImageUser, User);
