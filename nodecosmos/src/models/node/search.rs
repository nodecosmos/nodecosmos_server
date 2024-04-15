use charybdis::types::{BigInt, Timestamp};
use elasticsearch::{Elasticsearch, SearchParts};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use yrs::Uuid;

use crate::errors::NodecosmosError;
use crate::models::node::Node;
use crate::models::traits::ElasticIndex;
use crate::models::udts::Profile;

const PAGE_SIZE: i16 = 10;

#[derive(Deserialize, Serialize)]
pub struct IndexNode {
    pub id: Uuid,

    #[serde(rename = "rootId")]
    pub root_id: Uuid,

    #[serde(rename = "ancestorIds")]
    pub ancestor_ids: Option<Vec<Uuid>>,

    #[serde(rename = "ownerId")]
    pub owner_id: Uuid,

    pub title: String,

    #[serde(rename = "shortDescription")]
    pub short_description: Option<String>,

    pub description: Option<String>,

    #[serde(rename = "likesCount")]
    pub like_count: Option<BigInt>,

    #[serde(rename = "coverImageURL")]
    pub cover_image_url: Option<String>,

    #[serde(rename = "isRoot")]
    pub is_root: bool,

    #[serde(rename = "isPublic")]
    pub is_public: bool,

    #[serde(rename = "createdAt", default = "chrono::Utc::now")]
    pub created_at: Timestamp,

    pub owner: Profile,
}

#[derive(Deserialize)]
pub struct NodeSearchQuery {
    q: Option<String>,

    #[serde(default = "default_to_opt_0")]
    page: i16,
}

pub fn default_to_opt_0() -> i16 {
    0
}

pub struct NodeSearch<'a> {
    pub elastic_client: &'a Elasticsearch,
    pub node_search_query: &'a NodeSearchQuery,
}

impl<'a> NodeSearch<'a> {
    pub fn new(elastic_client: &'a Elasticsearch, node_search_query: &'a NodeSearchQuery) -> Self {
        Self {
            elastic_client,
            node_search_query,
        }
    }

    pub async fn index(&self) -> Result<Vec<IndexNode>, NodecosmosError> {
        let response = self
            .elastic_client
            .search(SearchParts::Index(&[Node::ELASTIC_IDX_NAME]))
            .body(self.search_json())
            .send()
            .await?;

        let response_body = response.json::<Value>().await?;

        let res = vec![];
        let hits = response_body["hits"]["hits"].as_array().unwrap_or(&res);

        let mut nodes: Vec<IndexNode> = Vec::new();
        for hit in hits {
            let document = serde_json::from_value(hit["_source"].clone())?;

            nodes.push(document);
        }

        Ok(nodes)
    }

    fn search_json(&self) -> Value {
        let mut data = json!({
            "query": {
                "bool": {
                    "must": [
                        { "term": { "isPublic": true } } // Only public nodes
                    ]
                }
            },
            "sort": [
                { "isRoot": { "order": "desc" } },
                { "likesCount": { "order": "desc" } },
                {
                    "_script": {
                        "type": "number",
                        "script": {
                            "lang": "painless",
                            "source": "doc['ancestorIds'].size()"
                        },
                        "order":"asc"
                    }
                },
                { "createdAt": { "order": "desc" } },
            ],
            "from": self.node_search_query.page * PAGE_SIZE,
            "size": PAGE_SIZE
        });

        if let Some(term) = &self.node_search_query.q {
            data["query"]["bool"]["should"] = json!([
                { "match": { "title": { "query": &term, "boost": 2 } } },
                { "match": { "description": &term } },
                { "match": { "owner.name": &term } },
                { "match": { "owner.username": &term } },
            ]);
            data["query"]["bool"]["minimum_should_match"] = json!(1);
            data["sort"] = json!([
                { "_score": { "order": "desc" } },
                { "isRoot": { "order": "desc" } },
                { "likesCount": { "order": "desc" } },
                { "createdAt": { "order": "desc" } }
            ]);
        }

        data
    }
}
