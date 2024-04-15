use charybdis::types::Uuid;
use colored::Colorize;
use elasticsearch::indices::{IndicesCreateParts, IndicesExistsParts, IndicesPutMappingParts};
use elasticsearch::Elasticsearch;
use log::{error, info};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::models::node::{Node, UpdateCoverImageNode, UpdateOwnerNode, UpdateTitleNode};
use crate::models::user::{UpdateBioUser, UpdateProfileImageUser, UpdateUser, User};

pub trait ElasticIndex {
    const ELASTIC_IDX_NAME: &'static str;

    fn settings_json() -> Value;
    fn mappings_json() -> Value;

    fn index_id(&self) -> String;

    async fn idx_exists(client: &Elasticsearch) -> bool {
        let response = client
            .indices()
            .exists(IndicesExistsParts::Index(&[Self::ELASTIC_IDX_NAME]))
            .send()
            .await
            .unwrap();

        let status = response.status_code().clone();

        status.is_success()
    }
}

pub trait BuildIndex: ElasticIndex {
    async fn build_index(client: &Elasticsearch);
}

impl<T: ElasticIndex> BuildIndex for T {
    async fn build_index(client: &Elasticsearch) {
        let response;

        if T::idx_exists(client).await {
            info!(
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
            info!(
                "{} {}",
                "Creating elastic index for".bright_green(),
                Self::ELASTIC_IDX_NAME.bright_yellow()
            );
            response = client
                .indices()
                .create(IndicesCreateParts::Index(Self::ELASTIC_IDX_NAME))
                .body(json!({
                    "settings": Self::settings_json(),
                    "mappings": Self::mappings_json()
                }))
                .send()
                .await;
        }

        let response = response.unwrap_or_else(|e| {
            panic!("Failed to send index request: {:#?}", e);
        });

        if !response.status_code().is_success() {
            error!("Failed Elasticsearch operation. Debug info: ...");

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
        json!({
            "dynamic": false,
            "properties": {
                "id": { "type": "keyword", "index": false },
                "rootId": { "type": "keyword", "index": false },
                 "ancestorIds": {
                    "type": "keyword",
                    "index": false
                },
                "ownerId": { "type": "keyword", "index": false },
                "title": { "type": "text", "analyzer": "english" },
                "shortDescription": { "type": "text", "index": false  },
                "description": {
                    "type": "text",
                    "analyzer": "english_with_html_strip",
                },
                "likesCount": { "type": "integer" },
                "isRoot": { "type": "boolean" },
                "isPublic": { "type": "boolean" },
                "createdAt": { "type": "date" },
                "coverImageUrl": { "type": "text", "index": false },
                "owner": {
                    "properties": {
                        "id": { "type": "keyword", "index": false },
                        "name": { "type": "text" },
                        "username": { "type": "keyword" },
                        "profileImageURL": { "type": "text", "index": false },
                    }
                },
            }
        })
    }

    fn index_id(&self) -> String {
        self.id.to_string()
    }
}

impl ElasticIndex for User {
    const ELASTIC_IDX_NAME: &'static str = "users";

    fn settings_json() -> Value {
        json!({
            "index": {
                "number_of_shards": 2,
                "number_of_replicas": 1
            }
        })
    }

    fn mappings_json() -> Value {
        json!({
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
        })
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

#[derive(Serialize, Deserialize)]
pub struct UpdateNodeDescriptionElasticIdx {
    pub id: Uuid,

    #[serde(rename = "shortDescription")]
    pub short_description: String,

    pub description: String,
}

#[derive(Serialize, Deserialize)]
pub struct UpdateLikesCountNodeElasticIdx {
    pub id: Uuid,

    #[serde(rename = "likesCount")]
    pub likes_count: i32,
}

// node
impl_elastic_index!(UpdateNodeDescriptionElasticIdx, Node);
impl_elastic_index!(UpdateTitleNode, Node);
impl_elastic_index!(UpdateCoverImageNode, Node);
impl_elastic_index!(UpdateLikesCountNodeElasticIdx, Node);
impl_elastic_index!(UpdateOwnerNode, Node);
// user
impl_elastic_index!(UpdateUser, User);
impl_elastic_index!(UpdateProfileImageUser, User);
impl_elastic_index!(UpdateBioUser, User);
