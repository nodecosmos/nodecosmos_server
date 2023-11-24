use crate::models::user::User;
use crate::services::elastic::index::ElasticIndex;
use serde_json::{json, Value};

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
}
