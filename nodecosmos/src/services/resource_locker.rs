use crate::errors::NodecosmosError;
use crate::models::node::Node;
use deadpool_redis::redis::AsyncCommands;
use deadpool_redis::{redis, Pool};
use std::sync::Arc;

#[derive(Clone)]
pub struct ResourceLocker {
    pool: Arc<Pool>,
}

impl ResourceLocker {
    pub fn new(pool: Arc<Pool>) -> Self {
        Self { pool }
    }

    pub async fn lock(&self, resource_id: &str, ttl: usize) -> Result<bool, NodecosmosError> {
        let mut connection = self.pool.get().await?;

        let namespaced_key = format!("LOCK:{}", resource_id);
        redis::cmd("SET")
            .arg(namespaced_key)
            .arg("1")
            .arg("NX")
            .arg("PX")
            .arg(ttl)
            .query_async(&mut *connection)
            .await?;
        Ok(true)
    }

    pub async fn is_locked(&self, resource_id: &str) -> Result<bool, NodecosmosError> {
        let namespaced_key = format!("LOCK:{}", resource_id);
        let mut connection = self.pool.get().await?;

        let res = connection.exists(namespaced_key).await?;

        Ok(res)
    }

    pub async fn unlock(&self, resource_id: &str) -> Result<bool, NodecosmosError> {
        let namespaced_key = format!("LOCK:{}", resource_id);
        let mut connection = self.pool.get().await?;

        let res = connection.del(namespaced_key).await?;

        Ok(res)
    }

    pub async fn check_node_lock(&self, node: &Node) -> Result<(), NodecosmosError> {
        if self.is_locked(&node.root_id.to_string()).await? {
            return Err(NodecosmosError::ResourceLocked(
                "Resource Locked: Reorder in progress. If issue persist contact support"
                    .to_string(),
            ));
        }

        Ok(())
    }
}
