use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::attachment::Attachment;
use crate::models::comment::Comment;
use crate::models::comment_thread::CommentThread;
use crate::models::description::Description;
use crate::models::udts::Profile;
use crate::models::user::User;
use crate::models::utils::impl_updated_at_cb;
use charybdis::callbacks::Callbacks;
use charybdis::macros::charybdis_model;
use charybdis::operations::Find;
use charybdis::types::{Double, Frozen, List, Text, Timestamp, Uuid};
use chrono::Utc;
use scylla::client::caching_session::CachingSession;
use serde::{Deserialize, Serialize};

#[charybdis_model(
    table_name = tasks,
    partition_keys = [branch_id],
    clustering_keys = [node_id, id],
)]
#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub branch_id: Uuid,
    pub node_id: Uuid,

    #[serde(default)]
    pub id: Uuid,

    pub section_id: Uuid,
    pub order_index: Double,
    pub title: Text,

    #[serde(default)]
    pub author_id: Uuid,

    #[serde(default)]
    pub author: Profile,

    #[serde(default)]
    pub assignee_ids: List<Uuid>,

    #[serde(default)]
    pub assignees: Frozen<List<Frozen<Profile>>>,

    #[serde(default = "chrono::Utc::now")]
    pub created_at: Timestamp,

    #[serde(default = "chrono::Utc::now")]
    pub updated_at: Timestamp,

    pub due_at: Option<Timestamp>,
    pub completed_at: Option<Timestamp>,
}

impl Callbacks for Task {
    type Extension = RequestData;
    type Error = NodecosmosError;

    async fn before_insert(&mut self, _session: &CachingSession, data: &RequestData) -> Result<(), Self::Error> {
        self.id = Uuid::new_v4();
        self.created_at = Utc::now();
        self.updated_at = Utc::now();
        self.author_id = data.current_user.id;
        self.author = (&data.current_user).into();

        Ok(())
    }

    async fn after_delete(
        &mut self,
        db_session: &CachingSession,
        _extension: &Self::Extension,
    ) -> Result<(), Self::Error> {
        CommentThread::delete_by_branch_id_and_object_id(self.branch_id, self.id)
            .execute(db_session)
            .await?;

        Comment::delete_by_branch_id_and_thread_id(self.branch_id, self.id)
            .execute(db_session)
            .await?;

        Attachment::delete_by_branch_id_and_node_id_and_object_id(self.branch_id, self.node_id, self.id)
            .execute(db_session)
            .await?;

        Description::delete_by_branch_id_and_object_id(self.branch_id, self.id)
            .execute(db_session)
            .await?;

        Ok(())
    }
}

partial_task!(
    UpdateAssigneesTask,
    branch_id,
    node_id,
    id,
    assignee_ids,
    assignees,
    updated_at
);

impl Callbacks for UpdateAssigneesTask {
    type Extension = RequestData;
    type Error = NodecosmosError;

    async fn before_update(&mut self, _session: &CachingSession, data: &RequestData) -> Result<(), Self::Error> {
        self.updated_at = Utc::now();

        let current_assignee_ids = self
            .find_by_primary_key()
            .execute(data.db_session())
            .await?
            .assignee_ids;

        let added_assignee_ids = self
            .assignee_ids
            .iter()
            .filter(|id| !current_assignee_ids.contains(id))
            .cloned()
            .collect::<Vec<Uuid>>();

        let removed_assignee_ids = current_assignee_ids
            .iter()
            .filter(|id| !self.assignee_ids.contains(id))
            .cloned()
            .collect::<Vec<Uuid>>();

        let added_users = User::find_by_ids(data.db_session(), &added_assignee_ids).await?;
        // let removed_users = User::find_by_ids(data.db_session(), &removed_assignee_ids).await?;

        // update assignees
        self.assignees = self
            .assignees
            .clone()
            .into_iter()
            .filter(|profile| !removed_assignee_ids.contains(&profile.id))
            .chain(added_users.iter().map(|user| user.into()))
            .collect::<Vec<Profile>>();

        // TODO: create a table to track assigned tasks per user
        // let mut update_assignees_batch = UpdateAssignedTasksUser::statement_batch();
        //
        // added_users.iter().for_each(|u| {});
        //
        // removed_users.iter().for_each(|u| {});

        // update_assignees_batch.execute(data.db_session()).await?;

        Ok(())
    }
}

partial_task!(
    UpdatePositionTask,
    branch_id,
    node_id,
    section_id,
    order_index,
    id,
    updated_at
);

impl_updated_at_cb!(UpdatePositionTask);

partial_task!(UpdateTitleTask, branch_id, node_id, id, title, updated_at);

impl_updated_at_cb!(UpdateTitleTask);

partial_task!(UpdateDueAtTask, branch_id, node_id, id, due_at, updated_at);

impl_updated_at_cb!(UpdateDueAtTask);
