use crate::errors::NodecosmosError;
use crate::models::attachment::Attachment;
use crate::models::comment::Comment;
use crate::models::comment_thread::CommentThread;
use crate::models::task::Task;
use crate::models::utils::{created_at_cb_fn, impl_updated_at_cb};
use charybdis::batch::{CharybdisBatch, ModelBatch};
use charybdis::callbacks::Callbacks;
use charybdis::macros::charybdis_model;
use charybdis::types::{Double, Text, Timestamp, Uuid};
use futures::StreamExt;
use scylla::client::caching_session::CachingSession;
use serde::{Deserialize, Serialize};

#[charybdis_model(
    table_name = task_sections,
    partition_keys = [branch_id],
    clustering_keys = [node_id, id],
)]
#[derive(Serialize, Deserialize, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TaskSection {
    pub title: Text,
    pub branch_id: Uuid,
    pub node_id: Uuid,

    #[serde(default)]
    pub id: Uuid,

    pub order_index: Double,

    #[serde(default = "chrono::Utc::now")]
    pub created_at: Timestamp,

    #[serde(default = "chrono::Utc::now")]
    pub updated_at: Timestamp,
}

impl Callbacks for TaskSection {
    type Extension = Option<()>;
    type Error = NodecosmosError;

    created_at_cb_fn!();

    async fn after_delete(
        &mut self,
        db_session: &CachingSession,
        _extension: &Self::Extension,
    ) -> Result<(), Self::Error> {
        // delete all tasks
        let mut delete_task_batch = Task::delete_batch();
        let mut task_ids = Vec::new();
        let mut tasks = Task::find_by_branch_id_and_node_id(self.branch_id, self.node_id)
            .execute(db_session)
            .await?;

        while let Some(task) = tasks.next().await {
            let task = task?;
            if task.section_id == self.id {
                delete_task_batch.append_delete(&task);
            }
            task_ids.push(task.id);
        }

        delete_task_batch.execute(db_session).await?;

        // delete all comment threads
        let mut delete_thread_batch = CharybdisBatch::new();
        task_ids.iter().for_each(|task_id| {
            let query = CommentThread::delete_by_branch_id_and_object_id(self.branch_id, *task_id);
            delete_thread_batch.append(query);
        });
        delete_thread_batch.execute(db_session).await?;

        // delete all comments
        let mut delete_comments_batch = CharybdisBatch::new();
        task_ids.iter().for_each(|task_id| {
            let query = Comment::delete_by_branch_id_and_thread_id(self.branch_id, *task_id);
            delete_comments_batch.append(query);
        });
        delete_comments_batch.execute(db_session).await?;

        // delete all attachments
        let mut delete_attachments_batch = CharybdisBatch::new();
        task_ids.iter().for_each(|task_id| {
            let query =
                Attachment::delete_by_branch_id_and_node_id_and_object_id(self.branch_id, self.node_id, *task_id);
            delete_attachments_batch.append(query);
        });
        delete_attachments_batch.execute(db_session).await?;

        Ok(())
    }
}

partial_task_section!(
    UpdateOrderIndexTaskSection,
    branch_id,
    node_id,
    id,
    order_index,
    updated_at
);

impl_updated_at_cb!(UpdateOrderIndexTaskSection);

partial_task_section!(UpdateTitleTaskSection, branch_id, node_id, id, title, updated_at);

impl_updated_at_cb!(UpdateTitleTaskSection);
