use crate::api::types::ActionTypes;
use crate::errors::NodecosmosError;
use crate::models::node::Node;
use deadpool_redis::redis::{AsyncCommands, Connection};
use deadpool_redis::{redis, Pool};

const LOCK_NAMESPACE: &str = "LOCK";

/// Resource Locker users redis to lock resources
#[derive(Clone)]
pub struct ResourceLocker {
    pool: Pool,
}

impl ResourceLocker {
    // const REDIS_INSTANCES: u8 = 1;
    const RETRY_LOCK_TIMEOUT: u64 = 500;
    const RESOURCE_LOCK_ERROR: NodecosmosError =
        NodecosmosError::ResourceLocked("Resource Locked. If issue persist contact support");

    pub fn new(pool: &Pool) -> Self {
        Self { pool: pool.clone() }
    }

    /// Lock complete resource
    pub async fn lock_resource(&self, resource_id: &str, ttl: usize) -> Result<bool, NodecosmosError> {
        let mut connection = self.pool.get().await?;
        let key = format!("{}:{}", LOCK_NAMESPACE, resource_id);

        if self.is_resource_locked(resource_id).await? {
            return Err(NodecosmosError::ResourceAlreadyLocked(format!(
                "Resource: {} is already locked",
                resource_id
            )));
        }

        redis::cmd("SET")
            .arg(key)
            .arg("1")
            .arg("NX")
            .arg("PX")
            .arg(ttl)
            .query_async(&mut *connection)
            .await
            .map_err(|e| {
                NodecosmosError::LockerError(format!("Failed to lock resource: {}! Error: {:?}", resource_id, e))
            })?;

        self.wait_for_write_replication(resource_id).await?;

        Ok(true)
    }

    /// To be used once we have multiple redis instances
    async fn wait_for_write_replication(&self, _resource_id: &str) -> Result<(), NodecosmosError> {
        // let mut connection = self.pool.get().await?;
        //
        // let wait_result: redis::RedisResult<usize> = redis::cmd("WAIT")
        //     .arg(Self::REDIS_INSTANCES) // Number of replicas to acknowledge the write.
        //     .arg(1000) // Timeout in milliseconds.
        //     .query_async(&mut *connection)
        //     .await;
        //
        // match wait_result {
        //     Ok(replicas) if replicas >= Self::REDIS_INSTANCES as usize => Ok(()),
        //     Ok(replicas) => Err(NodecosmosError::LockerError(format!(
        //         "Lock not sufficiently replicated for resource: {}. Replicas: {}",
        //         resource_id, replicas
        //     ))),
        //     Err(e) => Err(NodecosmosError::LockerError(format!(
        //         "WAIT command failed for resource: {}! Error: {:?}",
        //         resource_id, e
        //     ))),
        // }

        Ok(())
    }

    pub async fn lock_resource_actions(
        &self,
        resource_id: &str,
        actions: Vec<ActionTypes>,
        ttl: usize,
    ) -> Result<bool, NodecosmosError> {
        let mut connection = self.pool.get().await?;
        let key = format!("{}:{}", LOCK_NAMESPACE, resource_id);

        let mut pipe = redis::pipe();
        for action in actions {
            self.check_resource_action_unlocked(&action, resource_id).await?;

            pipe.cmd("SET")
                .arg(format!("{}:{}:{}", LOCK_NAMESPACE, action, resource_id))
                .arg("1")
                .arg("NX")
                .arg("PX")
                .arg(ttl);
        }
        pipe.cmd("SET").arg(key).arg("1").arg("NX").arg("PX").arg(ttl);

        pipe.query_async(&mut *connection).await.map_err(|e| {
            NodecosmosError::LockerError(format!("Failed to lock resource: {}! Error: {:?}", resource_id, e))
        })?;

        self.wait_for_write_replication(resource_id).await?;

        Ok(true)
    }

    async fn check_resource_action_unlocked(
        &self,
        resource_action: &ActionTypes,
        resource_id: &str,
    ) -> Result<(), NodecosmosError> {
        if self
            .is_resource_action_locked(&resource_action, resource_id.to_string())
            .await?
        {
            return Err(NodecosmosError::ResourceAlreadyLocked(format!(
                "Resource action: {} for resource: {} is already locked",
                resource_action, resource_id
            )));
        }

        Ok(())
    }

    pub async fn is_resource_locked(&self, resource_id: &str) -> Result<bool, NodecosmosError> {
        let mut connection = self.pool.get().await?;
        let key = format!("{}:{}", LOCK_NAMESPACE, resource_id);

        let res = connection.exists(key).await.map_err(|e| {
            NodecosmosError::LockerError(format!(
                "Failed to check if resource: {} is locked! Error: {:?}",
                resource_id, e
            ))
        })?;

        Ok(res)
    }

    pub async fn is_resource_action_locked(
        &self,
        resource_action: &ActionTypes,
        resource_id: String,
    ) -> Result<bool, NodecosmosError> {
        let mut connection = self.pool.get().await?;
        let key = format!("{}:{}:{}", LOCK_NAMESPACE, resource_action, resource_id);

        let res = connection.exists(key).await.map_err(|e| {
            NodecosmosError::LockerError(format!(
                "Failed to check if resource action: {} for resource: {} is locked! Error: {:?}",
                resource_action, resource_id, e
            ))
        })?;

        Ok(res)
    }

    pub async fn unlock_resource(&self, resource_id: &str) -> Result<bool, NodecosmosError> {
        let key = format!("LOCK:{}", resource_id);
        let mut connection = self.pool.get().await?;

        let res = connection.del(key).await.map_err(|e| {
            NodecosmosError::LockerError(format!("Failed to unlock resource: {}! Error: {:?}", resource_id, e))
        })?;

        Ok(res)
    }

    pub async fn unlock_resource_action(
        &self,
        resource_action: ActionTypes,
        resource_id: &str,
    ) -> Result<bool, NodecosmosError> {
        let mut connection = self.pool.get().await?;
        let key = format!("LOCK:{}:{}", resource_action, resource_id);

        let res = connection.del(key).await.map_err(|e| {
            NodecosmosError::LockerError(format!(
                "Failed to unlock resource action: {} for resource: {}! Error: {:?}",
                resource_action, resource_id, e
            ))
        })?;

        Ok(res)
    }

    pub async fn validate_resource_unlocked(&self, resource_id: &str) -> Result<(), NodecosmosError> {
        if self.is_resource_locked(resource_id).await? {
            return Err(Self::RESOURCE_LOCK_ERROR);
        }

        Ok(())
    }

    pub async fn validate_node_unlocked(&self, node: &Node, retry: bool) -> Result<(), NodecosmosError> {
        if self.is_resource_locked(&node.root_id.to_string()).await? {
            if retry {
                tokio::time::sleep(tokio::time::Duration::from_millis(Self::RETRY_LOCK_TIMEOUT)).await;

                // TODO: introduce recursion & retry count
                if self.is_resource_locked(&node.root_id.to_string()).await? {
                    return Err(Self::RESOURCE_LOCK_ERROR);
                }
            } else {
                return Err(Self::RESOURCE_LOCK_ERROR);
            }
        }

        Ok(())
    }

    pub async fn validate_action_unlocked(
        &self,
        node: &Node,
        action_type: ActionTypes,
        retry: bool,
    ) -> Result<(), NodecosmosError> {
        if self
            .is_resource_action_locked(&action_type, node.root_id.to_string())
            .await?
        {
            if retry {
                tokio::time::sleep(tokio::time::Duration::from_millis(Self::RETRY_LOCK_TIMEOUT)).await;

                // TODO: introduce recursion & retry count
                if self
                    .is_resource_action_locked(&action_type, node.root_id.to_string())
                    .await?
                {
                    return Err(Self::RESOURCE_LOCK_ERROR);
                }
            } else {
                return Err(Self::RESOURCE_LOCK_ERROR);
            }
        }

        Ok(())
    }
}
