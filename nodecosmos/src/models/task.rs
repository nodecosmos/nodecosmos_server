// use crate::api::data::RequestData;
// use crate::errors::NodecosmosError;
// use crate::models::udts::{AssignedTask, Profile};
// use crate::models::user::{UpdateAssignedTasksUser, User};
// use charybdis::batch::ModelBatch;
// use charybdis::callbacks::Callbacks;
// use charybdis::macros::charybdis_model;
// use charybdis::operations::Find;
// use charybdis::types::{Boolean, Double, Frozen, List, Timestamp, Uuid};
// use chrono::Utc;
// use scylla::client::caching_session::CachingSession;
// use serde::{Deserialize, Serialize};
//
// #[charybdis_model(
//     table_name = tasks,
//     partition_keys = [branch_id],
//     clustering_keys = [node_id, id],
// )]
// #[derive(Serialize, Deserialize, Clone, Default)]
// #[serde(rename_all = "camelCase")]
// pub struct Task {
//     pub branch_id: Uuid,
//     pub node_id: Uuid,
//     pub id: Uuid,
//     pub order_index: Double,
//     pub is_archived: Boolean,
//     pub is_completed: Boolean,
//     pub author_id: Uuid,
//     pub author: Profile,
//     pub assignee_ids: List<Uuid>,
//     pub assignees: Frozen<List<Frozen<Profile>>>,
//     pub created_at: Timestamp,
//     pub updated_at: Timestamp,
//     pub completed_at: Option<Timestamp>,
// }
//
// impl Callbacks for Task {
//     type Extension = RequestData;
//     type Error = NodecosmosError;
//
//     async fn before_insert(&mut self, _session: &CachingSession, data: &RequestData) -> Result<(), Self::Error> {
//         self.id = Uuid::new_v4();
//         self.created_at = Utc::now();
//         self.updated_at = Utc::now();
//         self.author_id = data.current_user.id;
//         self.author = Profile::init_from_current_user(&data.current_user);
//
//         Ok(())
//     }
// }
//
// partial_task!(
//     UpdateAssigneesTask,
//     branch_id,
//     node_id,
//     is_archived,
//     is_completed,
//     id,
//     assignee_ids,
//     assignees,
//     updated_at
// );
//
// impl Callbacks for UpdateAssigneesTask {
//     type Extension = RequestData;
//     type Error = NodecosmosError;
//
//     async fn before_update(&mut self, _session: &CachingSession, data: &RequestData) -> Result<(), Self::Error> {
//         self.updated_at = Utc::now();
//
//         let current_assignee_ids = self
//             .find_by_primary_key()
//             .execute(data.db_session())
//             .await?
//             .assignee_ids;
//
//         let added_assignee_ids = self
//             .assignee_ids
//             .iter()
//             .filter(|id| !current_assignee_ids.contains(id))
//             .cloned()
//             .collect::<Vec<Uuid>>();
//
//         let removed_assignee_ids = current_assignee_ids
//             .iter()
//             .filter(|id| !self.assignee_ids.contains(id))
//             .cloned()
//             .collect::<Vec<Uuid>>();
//
//         let added_users = User::find_by_ids(data.db_session(), &added_assignee_ids).await?;
//         let removed_users = User::find_by_ids(data.db_session(), &removed_assignee_ids).await?;
//
//         // update assignees
//         self.assignees = self
//             .assignees
//             .clone()
//             .into_iter()
//             .filter(|profile| !removed_assignee_ids.contains(&profile.id))
//             .chain(added_users.iter().map(|user| Profile::init(&user)))
//             .collect::<Vec<Profile>>();
//
//         // update user records
//         let mut update_assignees_batch = UpdateAssignedTasksUser::statement_batch();
//
//         added_users.iter().for_each(|u| {
//             update_assignees_batch.append_statement(
//                 User::PUSH_ASSIGNED_TASKS_QUERY,
//                 (
//                     vec![AssignedTask {
//                         node_id: self.node_id,
//                         branch_id: self.branch_id,
//                         task_id: self.id,
//                     }],
//                     u.id,
//                 ),
//             );
//         });
//
//         removed_users.iter().for_each(|u| {
//             update_assignees_batch.append_statement(
//                 User::PULL_ASSIGNED_TASKS_QUERY,
//                 (
//                     vec![AssignedTask {
//                         node_id: self.node_id,
//                         branch_id: self.branch_id,
//                         task_id: self.id,
//                     }],
//                     u.id,
//                 ),
//             );
//         });
//
//         update_assignees_batch.execute(data.db_session()).await?;
//
//         Ok(())
//     }
// }
//
// partial_task!(UpdateIsCompletedTask, branch_id, node_id, is_completed, id, updated_at);
//
// impl Callbacks for UpdateIsCompletedTask {
//     type Extension = RequestData;
//     type Error = NodecosmosError;
//
//     async fn before_update(&mut self, _session: &CachingSession, _: &RequestData) -> Result<(), Self::Error> {
//         self.updated_at = Utc::now();
//
//         Ok(())
//     }
// }
//
// partial_task!(UpdateIsArchivedTask, branch_id, node_id, is_archived, id, updated_at);
//
// impl Callbacks for UpdateIsArchivedTask {
//     type Extension = RequestData;
//     type Error = NodecosmosError;
//
//     async fn before_update(&mut self, _session: &CachingSession, _: &RequestData) -> Result<(), Self::Error> {
//         self.updated_at = Utc::now();
//
//         Ok(())
//     }
// }
//
// partial_task!(UpdateOrderIndexTask, branch_id, node_id, order_index, id, updated_at);
//
// impl Callbacks for UpdateOrderIndexTask {
//     type Extension = RequestData;
//     type Error = NodecosmosError;
//
//     async fn before_update(&mut self, _session: &CachingSession, _: &RequestData) -> Result<(), Self::Error> {
//         self.updated_at = Utc::now();
//
//         Ok(())
//     }
// }
