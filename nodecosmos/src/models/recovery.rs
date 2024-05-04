use crate::api::data::RequestData;
use crate::api::types::ActionTypes;
use crate::errors::NodecosmosError;
use crate::models::branch::merge::BranchMerge;
use crate::models::node::delete::NodeDelete;
use crate::models::node::reorder::Reorder;
use crate::resources::resource_locker::ResourceLocker;
use anyhow::Context;
use charybdis::macros::charybdis_model;
use charybdis::operations::{Delete, Insert, Update};
use charybdis::options::Consistency;
use charybdis::types::{Text, Timestamp, TinyInt, Uuid};
use futures::StreamExt;
use scylla::CachingSession;
use serde::{Deserialize, Serialize};

pub const RECOVERY_INTERVAL: u8 = 1;

#[derive(Deserialize)]
pub enum RecoveryObjectType {
    NodeDelete = 0,
    Reorder = 1,
    Merge = 2,
}

impl From<i8> for RecoveryObjectType {
    fn from(value: i8) -> Self {
        match value {
            0 => RecoveryObjectType::NodeDelete,
            1 => RecoveryObjectType::Reorder,
            2 => RecoveryObjectType::Merge,
            _ => panic!("Invalid recovery object type"),
        }
    }
}

#[charybdis_model(
    table_name = recoveries,
    partition_keys = [branch_id],
    clustering_keys = [object_type, id],
    global_secondary_indexes = [updated_at],
)]
#[derive(Default)]
pub struct Recovery {
    pub updated_at: Timestamp,
    pub branch_id: Uuid,
    pub object_type: TinyInt,
    pub id: Uuid,
    pub step: TinyInt,
    pub data: Text,
}

impl Recovery {
    pub fn new(branch_id: Uuid, object_type: RecoveryObjectType, id: Uuid, data: Text) -> Self {
        Self {
            updated_at: chrono::Utc::now(),
            step: 0,
            branch_id,
            object_type: object_type as i8,
            id,
            data,
        }
    }

    pub async fn run_recovery_task(data: &RequestData) -> Result<(), NodecosmosError> {
        let from_min_ago = chrono::Utc::now() - chrono::Duration::minutes(RECOVERY_INTERVAL as i64);
        // 3 minutes should be enough for main processes to recover from a crash.
        // If the process is still down after 3 minutes, we can assume that the process is not going to recover within
        // a main process lifetime, so we can recover the data from the log.
        let mut recoveries = find_recovery!("updated_at <= ? ALLOW FILTERING", (from_min_ago,))
            .consistency(Consistency::All)
            .execute(&data.db_session())
            .await?;

        while let Some(recovery) = recoveries.next().await {
            let recovery = recovery?;

            let locker_res = data
                .resource_locker()
                .validate_resource_action_unlocked(ActionTypes::Recover, recovery.id, recovery.branch_id, false)
                .await;

            if let Err(err) = locker_res {
                match err {
                    NodecosmosError::ResourceLocked(_) => {
                        // might be lock by another instance, so we skip this recovery
                        continue;
                    }
                    _ => return Err(err),
                }
            }

            data.resource_locker()
                .lock_resource_actions(
                    recovery.id,
                    recovery.branch_id,
                    &[ActionTypes::Recover],
                    ResourceLocker::FIVE_MINUTES,
                )
                .await?;

            match recovery.object_type.into() {
                RecoveryObjectType::NodeDelete => {
                    let mut node_delete: NodeDelete =
                        serde_json::from_str(&recovery.data).context("Failed to deserialize node delete data")?;
                    node_delete
                        .recover_from_log(&data)
                        .await
                        .context("Failed to recover NodeDelete from log")?;
                }
                RecoveryObjectType::Reorder => {
                    let mut reorder: Reorder =
                        serde_json::from_str(&recovery.data).context("Failed to deserialize reorder data")?;
                    reorder
                        .recover_from_log(&data)
                        .await
                        .context("Failed to recover Reorder from log")?;
                }
                RecoveryObjectType::Merge => {
                    let mut merge: BranchMerge =
                        serde_json::from_str(&recovery.data).context("Failed to deserialize branch merge data")?;
                    merge
                        .recover_from_log(&data)
                        .await
                        .context("Failed to recover BranchMerge from log")?;
                }
            }

            data.resource_locker()
                .unlock_resource_actions(recovery.id, recovery.branch_id, &[ActionTypes::Recover])
                .await?;
        }

        Ok(())
    }
}

/// Trait for recovering from a log. Before performing a SAGA operation, we serialize
/// the data structure that we are going to modify and store it in a recovery log. After each successful step, we
/// update the recovery log with the step number. If the operation fails, we can recover the data from the log and
/// continue from the last successful step. Note that log is used only if connection to the database is lost, so we
/// can have regular checks if recovery log is empty and if not, we can recover the data from the log.
pub trait RecoveryLog<'a>: Serialize + Deserialize<'a> {
    fn rec_id(&self) -> Uuid;

    fn rec_branch_id(&self) -> Uuid;

    fn rec_object_type(&self) -> RecoveryObjectType;

    async fn recover_from_log(&mut self, data: &RequestData) -> Result<(), NodecosmosError>;

    async fn create_recovery_log(&self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        let data = serde_json::to_string(self).expect("Failed to serialize branch merge data");
        let recovery = Recovery::new(self.rec_branch_id(), self.rec_object_type(), self.rec_id(), data.into());

        recovery.insert().execute(db_session).await?;

        Ok(())
    }

    async fn update_recovery_log_step(&self, db_session: &CachingSession, step: i8) -> Result<(), NodecosmosError> {
        let recovery = UpdateStepRecovery {
            branch_id: self.rec_branch_id(),
            object_type: self.rec_object_type() as i8,
            id: self.rec_id(),
            step,
        };

        recovery.update().execute(db_session).await?;

        Ok(())
    }

    async fn delete_recovery_log(&self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        DeleteRecovery {
            branch_id: self.rec_branch_id(),
            object_type: self.rec_object_type() as i8,
            id: self.rec_id(),
        }
        .delete()
        .execute(db_session)
        .await?;

        Ok(())
    }
}

partial_recovery!(UpdateStepRecovery, branch_id, object_type, id, step);

partial_recovery!(DeleteRecovery, branch_id, object_type, id);
