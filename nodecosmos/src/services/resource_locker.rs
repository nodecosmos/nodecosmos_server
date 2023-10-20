use crate::actions::types::ActionTypes;
use crate::errors::NodecosmosError;
use crate::models::node::Node;
use deadpool_redis::redis::AsyncCommands;
use deadpool_redis::{redis, Pool};
use std::sync::Arc;

const LOCK_NAMESPACE: &str = "LOCK";

#[derive(Clone)]
pub struct ResourceLocker {
    pool: Arc<Pool>,
}

impl ResourceLocker {
    pub fn new(pool: Arc<Pool>) -> Self {
        Self { pool }
    }

    /// Lock complete resource
    pub async fn lock_resource(&self, resource_id: &str, ttl: usize) -> Result<bool, NodecosmosError> {
        let mut connection = self.pool.get().await?;
        let key = format!("{}:{}", LOCK_NAMESPACE, resource_id);

        redis::cmd("SET")
            .arg(key)
            .arg("1")
            .arg("NX")
            .arg("PX")
            .arg(ttl)
            .query_async(&mut *connection)
            .await?;

        Ok(true)
    }

    /// Lock specific action on resource
    pub async fn lock_resource_action(
        &self,
        resource_action: ActionTypes,
        resource_id: &str,
        ttl: usize,
    ) -> Result<bool, NodecosmosError> {
        let mut connection = self.pool.get().await?;
        let key = format!("{}:{}:{}", LOCK_NAMESPACE, resource_action, resource_id);

        redis::cmd("SET")
            .arg(key)
            .arg("1")
            .arg("NX")
            .arg("PX")
            .arg(ttl)
            .query_async(&mut *connection)
            .await?;

        Ok(true)
    }

    pub async fn is_resource_locked(&self, resource_id: &str) -> Result<bool, NodecosmosError> {
        let mut connection = self.pool.get().await?;
        let key = format!("{}:{}", LOCK_NAMESPACE, resource_id);

        let res = connection.exists(key).await?;

        Ok(res)
    }

    pub async fn is_resource_action_locked(
        &self,
        resource_action: ActionTypes,
        resource_id: &str,
    ) -> Result<bool, NodecosmosError> {
        let mut connection = self.pool.get().await?;
        let key = format!("{}:{}:{}", LOCK_NAMESPACE, resource_action, resource_id);

        let res = connection.exists(key).await?;

        Ok(res)
    }

    pub async fn unlock_resource(&self, resource_id: &str) -> Result<bool, NodecosmosError> {
        let key = format!("LOCK:{}", resource_id);
        let mut connection = self.pool.get().await?;

        let res = connection.del(key).await?;

        Ok(res)
    }

    pub async fn unlock_resource_action(
        &self,
        resource_action: ActionTypes,
        resource_id: &str,
    ) -> Result<bool, NodecosmosError> {
        let mut connection = self.pool.get().await?;
        let key = format!("LOCK:{}:{}", resource_action, resource_id);

        let res = connection.del(key).await?;

        Ok(res)
    }

    pub async fn check_resource_lock(&self, resource_id: &str) -> Result<(), NodecosmosError> {
        if self.is_resource_locked(resource_id).await? {
            return Err(NodecosmosError::ResourceLocked("Resource Locked!".to_string()));
        }

        Ok(())
    }

    pub async fn check_node_lock(&self, node: &Node) -> Result<(), NodecosmosError> {
        if self.is_resource_locked(&node.root_id.to_string()).await? {
            return Err(NodecosmosError::ResourceLocked(
                "Resource Locked: Reorder in progress. If issue persist contact support".to_string(),
            ));
        }

        Ok(())
    }

    pub async fn check_node_action_lock(&self, action_type: ActionTypes, node: &Node) -> Result<(), NodecosmosError> {
        if self
            .is_resource_action_locked(action_type, &node.root_id.to_string())
            .await?
        {
            return Err(NodecosmosError::ResourceLocked(
                "Resource Locked: Reorder in progress. If issue persist contact support".to_string(),
            ));
        }

        Ok(())
    }
}
