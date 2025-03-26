use crate::api::current_user::OptCurrentUser;
use crate::errors::NodecosmosError;
use crate::models::node::Node;
use crate::models::traits::ElasticIndex;
use crate::models::udts::Profile;
use charybdis::types::{BigInt, Int, Timestamp, Uuid};
use elasticsearch::{Elasticsearch, SearchParts};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashSet;

const PAGE_SIZE: i16 = 20;

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexNode {
    pub id: Uuid,
    pub branch_id: Uuid,
    pub root_id: Uuid,
    pub ancestor_ids: Option<Vec<Uuid>>,
    pub owner_id: Uuid,
    pub creator_id: Option<Uuid>,
    pub title: String,
    pub short_description: Option<String>,
    pub description: Option<String>,
    pub like_count: Option<BigInt>,
    pub contribution_requests_count: Option<Int>,
    pub threads_count: Option<Int>,
    pub cover_image_url: Option<String>,
    pub is_root: bool,
    pub is_public: bool,
    pub editor_ids: Option<HashSet<Uuid>>,

    #[serde(default = "chrono::Utc::now")]
    pub created_at: Timestamp,

    pub owner: Profile,
    pub creator: Option<Profile>,
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
    pub opt_cu: &'a OptCurrentUser,
}

impl<'a> NodeSearch<'a> {
    pub fn new(
        elastic_client: &'a Elasticsearch,
        node_search_query: &'a NodeSearchQuery,
        opt_cu: &'a OptCurrentUser,
    ) -> Self {
        Self {
            elastic_client,
            node_search_query,
            opt_cu,
        }
    }

    pub async fn index(&self) -> Result<Vec<IndexNode>, NodecosmosError> {
        let response = self
            .elastic_client
            .search(SearchParts::Index(&[Node::ELASTIC_IDX_NAME]))
            .body(self.search_json())
            .send()
            .await?;

        let mut response_body = response.json::<Value>().await?;

        let mut res = vec![];
        let hits = response_body["hits"]["hits"].as_array_mut().unwrap_or(&mut res);

        let mut nodes: Vec<IndexNode> = Vec::new();
        for hit in hits {
            let document = serde_json::from_value(hit["_source"].take())?;

            nodes.push(document);
        }

        Ok(nodes)
    }

    fn search_json(&self) -> Value {
        // Base payload with sorting and pagination.
        let mut data = json!({
            "sort": [
                { "likeCount": { "order": "desc" } },
                { "isRoot": { "order": "desc" } },
                {
                    "_script": {
                        "type": "number",
                        "script": {
                            "lang": "painless",
                            "source": "doc['ancestorIds'].size()"
                        },
                        "order": "asc"
                    }
                },
                { "createdAt": { "order": "desc" } }
            ],
            "from": self.node_search_query.page * PAGE_SIZE,
            "size": PAGE_SIZE
        });

        // Initialize a base bool query.
        let mut query = json!({ "bool": {} });

        // If there's a search term, add the should clauses and adjust the sort.
        if let Some(term) = &self.node_search_query.q {
            query["bool"]["should"] = json!([
                { "match": { "title": { "query": term, "boost": 2 } } },
                { "match": { "description": term } },
                { "match": { "owner.name": term } },
                { "match": { "owner.username": term } }
            ]);
            query["bool"]["minimum_should_match"] = json!(1);
            data["sort"] = json!([
                { "_score": { "order": "desc" } },
                { "isRoot": { "order": "desc" } },
                { "likeCount": { "order": "desc" } },
                { "createdAt": { "order": "desc" } }
            ]);
        }

        // Build the filter based on whether a current user exists.
        let filter = if let Some(current_user) = &self.opt_cu.0 {
            let user_id = current_user.id.to_string();
            json!({
                "bool": {
                    "should": [
                        { "term": { "isPublic": true } },
                        { "term": { "ownerId": user_id } },
                        { "term": { "editorIds": user_id } }
                    ],
                    "minimum_should_match": 1
                }
            })
        } else {
            json!({
                "bool": {
                    "should": [
                        { "term": { "isPublic": true } }
                    ],
                    "minimum_should_match": 1
                }
            })
        };

        // Attach the filter to our bool query.
        query["bool"]["filter"] = filter;

        // Set the query on the payload.
        data["query"] = query;
        data
    }
}
