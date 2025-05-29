use crate::errors::NodecosmosError;
use crate::resources::resource::RedisClusterManager;
use actix_session::storage::{generate_session_key, LoadError, SaveError, SessionKey, SessionStore, UpdateError};
use redis::{AsyncCommands, FromRedisValue, Value};
use std::collections::HashMap;
use std::sync::Arc;
use time::Duration;

#[derive(Clone)]
struct CacheConfiguration {
    cache_keygen: Arc<dyn Fn(&str) -> String + Send + Sync>,
}

impl Default for CacheConfiguration {
    fn default() -> Self {
        Self {
            cache_keygen: Arc::new(str::to_owned),
        }
    }
}

#[derive(Clone)]
pub struct RedisClusterSessionStore {
    pub pool: deadpool::managed::Pool<RedisClusterManager>,
    // Add any other config you need, e.g. a function to compute cache keys, etc.
    configuration: CacheConfiguration,
}

impl RedisClusterSessionStore {
    pub fn new(pool: deadpool::managed::Pool<RedisClusterManager>) -> Self {
        Self {
            pool,
            configuration: CacheConfiguration::default(),
        }
    }

    /// Helper method to run a Redis command using a cluster connection.
    async fn execute_command<T: FromRedisValue>(&self, cmd: &redis::Cmd) -> Result<T, NodecosmosError> {
        // 1) Get a cluster connection from the pool.
        let mut conn = self.pool.get().await?;
        // 2) Execute the command.

        cmd.query_async(&mut *conn)
            .await
            .map_err(|e| NodecosmosError::InternalServerError(format!("Failed to get valkey connection: {}", e)))
    }
}

impl SessionStore for RedisClusterSessionStore {
    async fn load(&self, session_key: &SessionKey) -> Result<Option<HashMap<String, String>>, LoadError> {
        let cache_key = (self.configuration.cache_keygen)(session_key.as_ref());

        let value: Option<String> = self
            .execute_command(redis::cmd("GET").arg(&[&cache_key]))
            .await
            .map_err(Into::into)
            .map_err(LoadError::Other)?;

        match value {
            None => Ok(None),
            Some(value) => Ok(serde_json::from_str(&value)
                .map_err(Into::into)
                .map_err(LoadError::Deserialization)?),
        }
    }

    async fn save(&self, session_state: HashMap<String, String>, ttl: &Duration) -> Result<SessionKey, SaveError> {
        let body = serde_json::to_string(&session_state)
            .map_err(Into::into)
            .map_err(SaveError::Serialization)?;
        let session_key = generate_session_key();
        let cache_key = (self.configuration.cache_keygen)(session_key.as_ref());

        self.execute_command::<()>(
            redis::cmd("SET")
                .arg(&[
                    &cache_key, // key
                    &body,      // value
                    "NX",       // only set the key if it does not already exist
                    "EX",       // set expiry / TTL
                ])
                .arg(
                    ttl.whole_seconds(), // EXpiry in seconds
                ),
        )
        .await
        .map_err(Into::into)
        .map_err(SaveError::Other)?;

        Ok(session_key)
    }

    async fn update(
        &self,
        session_key: SessionKey,
        session_state: HashMap<String, String>,
        ttl: &Duration,
    ) -> Result<SessionKey, UpdateError> {
        let body = serde_json::to_string(&session_state)
            .map_err(Into::into)
            .map_err(UpdateError::Serialization)?;

        let cache_key = (self.configuration.cache_keygen)(session_key.as_ref());

        let v: Value = self
            .execute_command(redis::cmd("SET").arg(&[
                &cache_key,
                &body,
                "XX", // XX: Only set the key if it already exist.
                "EX", // EX: set expiry
                &format!("{}", ttl.whole_seconds()),
            ]))
            .await
            .map_err(Into::into)
            .map_err(UpdateError::Other)?;

        match v {
            Value::Nil => {
                // The SET operation was not performed because the XX condition was not verified.
                // This can happen if the session state expired between the load operation and the
                // update operation. Unlucky, to say the least. We fall back to the save routine
                // to ensure that the new key is unique.
                self.save(session_state, ttl).await.map_err(|err| match err {
                    SaveError::Serialization(err) => UpdateError::Serialization(err),
                    SaveError::Other(err) => UpdateError::Other(err),
                })
            }
            Value::Int(_) | Value::Okay | Value::SimpleString(_) => Ok(session_key),
            val => Err(UpdateError::Other(anyhow::anyhow!(
                "Failed to update session state. {:?}",
                val
            ))),
        }
    }

    async fn update_ttl(&self, session_key: &SessionKey, ttl: &Duration) -> anyhow::Result<()> {
        let cache_key = (self.configuration.cache_keygen)(session_key.as_ref());

        self.pool
            .get()
            .await?
            .expire::<_, ()>(&cache_key, ttl.whole_seconds())
            .await?;

        Ok(())
    }

    async fn delete(&self, session_key: &SessionKey) -> Result<(), anyhow::Error> {
        let cache_key = (self.configuration.cache_keygen)(session_key.as_ref());

        self.execute_command::<()>(redis::cmd("DEL").arg(&[&cache_key]))
            .await
            .map_err(Into::into)
            .map_err(UpdateError::Other)?;

        Ok(())
    }
}
