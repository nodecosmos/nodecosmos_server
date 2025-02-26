use crate::api::types::ActionTypes;
use crate::errors::NodecosmosError;
use crate::resources::resource::RedisClusterManager;
use charybdis::types::Uuid;
use redis::AsyncCommands;

const LOCK_NAMESPACE: &str = "LOCK";

/// Resource Locker users redis to lock resources
#[derive(Clone)]
pub struct ResourceLocker {
    pool: deadpool::managed::Pool<RedisClusterManager>,
    replicas: u8,
}

impl ResourceLocker {
    pub const ONE_HOUR: usize = 1000 * 60 * 60;
    pub const TWO_SECONDS: usize = 2000;
    pub const FIVE_MINUTES: usize = 1000 * 60 * 5;

    const RETRY_LOCK_TIMEOUT: u64 = 1000;
    const RESOURCE_LOCK_ERROR: NodecosmosError =
        NodecosmosError::ResourceLocked("Resource Locked. If issue persist contact support");

    pub fn new(pool: &deadpool::managed::Pool<RedisClusterManager>, replicas: u8) -> Self {
        Self {
            pool: pool.clone(),
            replicas,
        }
    }

    /// Lock complete resource
    pub async fn lock_resource(&self, id: Uuid, branch_id: Uuid, ttl: usize) -> Result<(), NodecosmosError> {
        let mut connection = self.pool.get().await?;

        if let Err(e) = self.validate_resource_unlocked(id, branch_id, true).await {
            return Err(NodecosmosError::ResourceAlreadyLocked(format!(
                "Resource: {} is already locked: Error: {}",
                id, e
            )));
        }

        redis::cmd("SET")
            .arg(self.key(id, branch_id))
            .arg("1")
            .arg("NX")
            .arg("PX")
            .arg(ttl)
            .query_async::<()>(&mut *connection)
            .await
            .map_err(|e| NodecosmosError::LockerError(format!("Failed to lock resource: {}! Error: {:?}", id, e)))?;

        self.wait_for_write_replication()
            .await
            .map_err(|e| NodecosmosError::LockerError(format!("Failed to lock resource: {}! Error: {:?}", id, e)))?;

        Ok(())
    }

    pub async fn lock_resource_actions(
        &self,
        id: Uuid,
        branch_id: Uuid,
        actions: &[ActionTypes],
        ttl: usize,
    ) -> Result<(), NodecosmosError> {
        let mut connection = self.pool.get().await?;

        if id == Uuid::default() {
            return Ok(());
        }

        // Locking particular actions requires resource to be unlocked
        self.validate_resource_unlocked(id, branch_id, true)
            .await
            .map_err(|e| {
                NodecosmosError::ResourceAlreadyLocked(format!(
                    "[lock_resource_actions] Resource  is already locked. Error: {}",
                    e
                ))
            })?;

        let mut pipe = redis::pipe();
        for action in actions {
            pipe.cmd("SET")
                .arg(self.action_key(action, id, branch_id))
                .arg("1")
                .arg("NX")
                .arg("PX")
                .arg(ttl);
        }

        pipe.query_async::<()>(&mut *connection).await.map_err(|e| {
            NodecosmosError::LockerError(format!("[query_async] Failed to lock resource: {}! Error: {:?}", id, e))
        })?;

        self.wait_for_write_replication().await.map_err(|e| {
            NodecosmosError::LockerError(format!(
                "[wait_for_write_replication] Failed to lock resource: {}! Error: {:?}",
                id, e
            ))
        })?;

        Ok(())
    }

    pub async fn unlock_resource_actions(
        &self,
        id: Uuid,
        branch_id: Uuid,
        actions: &[ActionTypes],
    ) -> Result<(), NodecosmosError> {
        let mut connection = self.pool.get().await?;
        let mut pipe = redis::pipe();

        for action in actions {
            pipe.cmd("DEL").arg(self.action_key(action, id, branch_id));
        }

        pipe.query_async::<()>(&mut *connection)
            .await
            .map_err(|e| NodecosmosError::LockerError(format!("Failed to unlock resource: {}! Error: {:?}", id, e)))?;

        Ok(())
    }

    pub async fn unlock_resource(&self, id: Uuid, branch_id: Uuid) -> Result<bool, NodecosmosError> {
        let mut connection = self.pool.get().await?;

        let res = connection
            .del(self.key(id, branch_id))
            .await
            .map_err(|e| NodecosmosError::LockerError(format!("Failed to unlock resource: {}! Error: {:?}", id, e)))?;

        Ok(res)
    }

    pub async fn unlock_resource_action(
        &self,
        action: ActionTypes,
        id: Uuid,
        branch_id: Uuid,
    ) -> Result<bool, NodecosmosError> {
        let mut connection = self.pool.get().await?;

        let res = connection
            .del(self.action_key(&action, id, branch_id))
            .await
            .map_err(|e| {
                NodecosmosError::LockerError(format!(
                    "Failed to unlock resource action: {} for resource: {}! Error: {:?}",
                    action, id, e
                ))
            })?;

        Ok(res)
    }

    pub async fn validate_resource_unlocked(
        &self,
        id: Uuid,
        branch_id: Uuid,
        retry: bool,
    ) -> Result<(), NodecosmosError> {
        if self.is_resource_locked(id, branch_id).await? && retry {
            // try again
            tokio::time::sleep(tokio::time::Duration::from_millis(Self::RETRY_LOCK_TIMEOUT)).await;

            if self.is_resource_locked(id, branch_id).await? {
                return Err(Self::RESOURCE_LOCK_ERROR);
            }
        }

        Ok(())
    }

    pub async fn validate_resource_action_unlocked(
        &self,
        action: ActionTypes,
        id: Uuid,
        branch_id: Uuid,
        retry: bool,
    ) -> Result<(), NodecosmosError> {
        if self.is_resource_action_locked(&action, id, branch_id).await? && retry {
            // try again
            tokio::time::sleep(tokio::time::Duration::from_millis(Self::RETRY_LOCK_TIMEOUT)).await;

            if self.is_resource_action_locked(&action, id, branch_id).await? {
                return Err(Self::RESOURCE_LOCK_ERROR);
            }
        }

        Ok(())
    }

    fn key(&self, id: Uuid, branch_id: Uuid) -> String {
        format!("{}:{{{}}}:{}", LOCK_NAMESPACE, id, branch_id)
    }

    fn action_key(&self, action: &ActionTypes, id: Uuid, branch_id: Uuid) -> String {
        // Use a hash tag based on the resource id to force all keys into the same slot.
        // This ensures that all keys like "{resource-id}:<action>" hash to the same slot.
        format!("{}:{{{}}}:{}:{}", LOCK_NAMESPACE, id, action, branch_id)
    }

    /// To be used once we have multiple redis instances
    async fn wait_for_write_replication(&self) -> Result<(), NodecosmosError> {
        let mut connection = self.pool.get().await?;

        let wait_result: redis::RedisResult<usize> = redis::cmd("WAIT")
            .arg(self.replicas) // Number of replicas to acknowledge the write.
            .arg(1000) // Timeout in milliseconds.
            .query_async(&mut *connection)
            .await;

        match wait_result {
            Ok(replicas) if replicas >= self.replicas as usize => Ok(()),
            Ok(replicas) => Err(NodecosmosError::LockerError(format!(
                "Lock not sufficiently replicated! Replicas: {}",
                replicas
            ))),
            Err(e) => Err(NodecosmosError::LockerError(format!(
                "WAIT command failed! Error: {:?}",
                e
            ))),
        }
    }

    async fn is_resource_locked(&self, id: Uuid, branch_id: Uuid) -> Result<bool, NodecosmosError> {
        let mut connection = self.pool.get().await?;

        let res = connection.exists(self.key(id, branch_id)).await.map_err(|e| {
            NodecosmosError::LockerError(format!(
                "Failed to check if resource: {} is locked! Error: {:?}",
                self.key(id, branch_id),
                e
            ))
        })?;

        Ok(res)
    }

    async fn is_resource_action_locked(
        &self,
        action: &ActionTypes,
        id: Uuid,
        branch_id: Uuid,
    ) -> Result<bool, NodecosmosError> {
        let mut connection = self.pool.get().await?;

        let res = connection
            .exists(self.action_key(action, id, branch_id))
            .await
            .map_err(|e| {
                NodecosmosError::LockerError(format!(
                    "Failed to check if resource action: {} for resource: {} is locked! Error: {:?}",
                    action, id, e
                ))
            })?;

        Ok(res)
    }
}
