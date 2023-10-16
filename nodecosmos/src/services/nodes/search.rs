use crate::errors::NodecosmosError;
use crate::models::node::{BaseNode, Node};
use elasticsearch::{Elasticsearch, SearchParts};
use serde::Deserialize;
use serde_json::{json, Value};

const PAGE_SIZE: i16 = 10;

#[derive(Deserialize)]
pub struct NodeSearchQuery {
    q: Option<String>,

    #[serde(default = "default_to_0")]
    page: i16,
}

pub fn default_to_0() -> i16 {
    0
}

pub struct NodeSearchService<'a> {
    pub elastic_client: &'a Elasticsearch,
    pub node_search_query: &'a NodeSearchQuery,
}

impl<'a> NodeSearchService<'a> {
    pub fn new(elastic_client: &'a Elasticsearch, node_search_query: &'a NodeSearchQuery) -> Self {
        Self {
            elastic_client,
            node_search_query,
        }
    }

    pub async fn index(&self) -> Result<Vec<BaseNode>, NodecosmosError> {
        let response = self
            .elastic_client
            .search(SearchParts::Index(&[Node::ELASTIC_IDX_NAME]))
            .body(self.search_json())
            .send()
            .await?;

        let response_body = response.json::<Value>().await?;

        let res = vec![];
        let hits = response_body["hits"]["hits"].as_array().unwrap_or(&res);

        let mut nodes: Vec<BaseNode> = Vec::new();
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
                        { "term": { "isPublic": true } }
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
                { "match": { "description": &term } }
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
