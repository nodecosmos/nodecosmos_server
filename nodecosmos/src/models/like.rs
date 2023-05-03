use chrono::Utc;

use crate::models::likes_count::LikesCount;
use charybdis::*;

// CQL limitation is to have counters in a separate table
// https://docs.datastax.com/en/cql-oss/3.3/cql/cql_using/useCounters.html

#[derive(Debug, Deserialize)]
pub enum ObjectTypes {
    Node,
}

impl ObjectTypes {
    pub fn to_string(&self) -> String {
        match self {
            ObjectTypes::Node => "Node".to_string(),
        }
    }

    pub fn from_string(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Node" => Ok(ObjectTypes::Node),
            _ => Err(()),
        }
    }
}

#[partial_model_generator]
#[charybdis_model(
    table_name = "likes",
    partition_keys = ["object_id"],
    clustering_keys = ["user_id"],
    secondary_indexes = []
)]
pub struct Like {
    pub object_id: Uuid,
    pub object_type: Text,
    pub user_id: Uuid,
    pub username: Text,
    pub created_at: Option<Timestamp>,
    pub updated_at: Option<Timestamp>,
}

impl Callbacks for Like {
    async fn before_delete(&mut self, session: &CachingSession) -> Result<(), CharybdisError> {
        LikesCount::decrement(&session, self.object_id).await?;

        Ok(())
    }

    async fn before_insert(&mut self, session: &CachingSession) -> Result<(), CharybdisError> {
        self.created_at = Some(Utc::now());
        self.updated_at = Some(Utc::now());

        let existing_like_query = find_like_query!("object_id = ? AND user_id = ?");
        let existing_like =
            Like::find_one(session, existing_like_query, (self.object_id, self.user_id))
                .await
                .ok();

        if existing_like.is_some() {
            return Err(CharybdisError::ValidationError((
                "user".to_string(),
                "already liked!".to_string(),
            )));
        }

        Ok(())
    }
}

impl Like {
    pub async fn like(
        session: &CachingSession,
        object_id: Uuid,
        object_type: ObjectTypes,
        user_id: Uuid,
        username: String,
    ) -> Result<(), CharybdisError> {
        let object_type = object_type.to_string();

        let mut like = Like {
            object_id,
            object_type,
            user_id,
            username,
            ..Default::default()
        };

        like.insert_cb(session).await?;
        LikesCount::increment(&session, object_id).await?;

        Ok(())
    }

    async fn set_cached_likes_count(&self) {
        match ObjectTypes::from_string(self.object_type.as_str()) {
            Ok(ObjectTypes::Node) => {}
            _ => println!("Invalid color!"),
        }
    }
}
