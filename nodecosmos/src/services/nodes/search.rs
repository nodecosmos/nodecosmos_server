use crate::errors::NodecosmosError;
use crate::models::node::{BaseNode, Node};
use elasticsearch::{Elasticsearch, SearchParts};
use serde_json::{json, Value};

pub struct NodeSearchService<'a> {
    pub elastic_client: &'a Elasticsearch,
}

impl<'a> NodeSearchService<'a> {
    pub fn new(elastic_client: &'a Elasticsearch) -> Self {
        Self { elastic_client }
    }

    pub async fn index(&self) -> Result<Vec<BaseNode>, NodecosmosError> {
        let query = json!({
         "query": {
            "bool": {
              "must": [
                { "term": { "isPublic": true } },
              ]
            }
          },
          "sort": [
            { "isRoot": { "order": "desc" } },
            { "likesCount": { "order": "desc" } },
            { "createdAt": { "order": "desc" } }
          ],
          "from": 0,
          "size": 10
        });

        let response = self
            .elastic_client
            .search(SearchParts::Index(&[Node::ELASTIC_IDX_NAME]))
            .body(query)
            .send()
            .await?;

        let response_body = response.json::<Value>().await?;

        let hits = response_body["hits"]["hits"].as_array().unwrap();

        let mut nodes = vec![];
        for hit in hits {
            let document: BaseNode = serde_json::from_value(hit["_source"].clone()).unwrap();

            nodes.push(document);
        }

        Ok(nodes)
    }
}
