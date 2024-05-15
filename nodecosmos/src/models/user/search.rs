use crate::api::data::RequestData;
use charybdis::types::{Boolean, Timestamp};
use elasticsearch::{Elasticsearch, SearchParts};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use yrs::Uuid;

use crate::errors::NodecosmosError;
use crate::models::traits::ElasticIndex;
use crate::models::user::User;

const PAGE_SIZE: i16 = 10;

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchUser {
    pub id: Uuid,
    pub username: String,
    pub email: String,
    pub first_name: String,
    pub last_name: String,
    pub bio: Option<String>,
    pub profile_image_url: Option<String>,

    #[serde(default)]
    pub is_confirmed: Boolean,

    #[serde(default)]
    pub is_blocked: Boolean,

    #[serde(default = "chrono::Utc::now")]
    pub created_at: Timestamp,

    #[serde(default = "chrono::Utc::now")]
    pub updated_at: Timestamp,
}

#[derive(Deserialize)]
pub struct UserSearchQuery {
    query: String,

    #[serde(default = "default_to_opt_0")]
    page: i16,
}

pub fn default_to_opt_0() -> i16 {
    0
}

pub struct UserSearch<'a> {
    pub elastic_client: &'a Elasticsearch,
    pub q: &'a UserSearchQuery,
}

impl<'a> UserSearch<'a> {
    pub fn new(elastic_client: &'a Elasticsearch, q: &'a UserSearchQuery) -> Self {
        Self { elastic_client, q }
    }

    pub async fn index(&self) -> Result<Vec<SearchUser>, NodecosmosError> {
        let response = self
            .elastic_client
            .search(SearchParts::Index(&[User::ELASTIC_IDX_NAME]))
            .body(self.search_json())
            .send()
            .await?;

        let mut response_body = response.json::<Value>().await?;

        let mut res = vec![];
        let hits = response_body["hits"]["hits"].as_array_mut().unwrap_or(&mut res);

        let mut users: Vec<SearchUser> = Vec::new();
        for hit in hits {
            let document = serde_json::from_value(hit["_source"].take())?;

            users.push(document);
        }

        Ok(users)
    }

    fn search_json(&self) -> Value {
        let mut data = json!({
            // "query": {
            //     "bool": {
            //         "must": [
            //             { "term": { "isConfirmed": true } } // Only confirmed users
            //         ]
            //     }
            // },
            "from": self.q.page * PAGE_SIZE,
            "size": PAGE_SIZE
        });

        data["query"]["bool"]["should"] = json!([
            { "match": { "username": { "query": &&self.q.query, "boost": 2 } } },
            { "match": { "firstName": &&self.q.query } },
            { "match": { "lastName": &&self.q.query } },
        ]);
        data["query"]["bool"]["minimum_should_match"] = json!(1);

        data
    }
}

impl User {
    pub async fn search(data: RequestData, query: &UserSearchQuery) -> Result<Vec<SearchUser>, NodecosmosError> {
        let user_search = UserSearch::new(data.elastic_client(), query);

        user_search.index().await
    }
}
