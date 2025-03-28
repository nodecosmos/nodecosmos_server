// use crate::api::current_user::OptCurrentUser;
// use crate::api::data::RequestData;
// use crate::api::types::Response;
// use crate::models::description::Description;
// use crate::models::node::AuthNode;
// use crate::models::task::{
//     Task, UpdateAssigneesTask, UpdateIsArchivedTask, UpdateIsCompletedTask, UpdateOrderIndexTask,
// };
// use crate::models::task_section::TaskSection;
// use actix_web::{get, post, put, web, HttpResponse};
// use charybdis::operations::{InsertWithCallbacks, UpdateWithCallbacks};
// use charybdis::types::Uuid;
// use serde_json::json;
//
// #[post("/task_section")]
// pub async fn create_task_section(data: RequestData, task_section: web::Json<TaskSection>) -> Response {
//     let mut task_section = task_section.into_inner();
//
//     AuthNode::auth_update(
//         &data,
//         task_section.branch_id,
//         task_section.node_id,
//         task_section.branch_id,
//     )
//     .await?;
//
//     task_section.insert_cb(&None).execute(data.db_session()).await?;
//
//     Ok(HttpResponse::Created().json(task_section))
// }
//
// #[get("/{branch_id}/{node_id}/{is_archived}/{is_completed}")]
// pub async fn get_task_sections(
//     data: RequestData,
//     path: web::Path<(Uuid, Uuid, Option<bool>, Option<bool>)>,
// ) -> Response {
//     let (branch_id, node_id, maybe_is_archived, maybe_is_completed) = path.into_inner();
//     let is_archived = maybe_is_archived.unwrap_or_default();
//     let is_completed = maybe_is_completed.unwrap_or_default();
//
//     AuthNode::auth_view(
//         data.db_session(),
//         &OptCurrentUser(Some(data.current_user.clone())),
//         branch_id,
//         node_id,
//         branch_id,
//     )
//     .await?;
//
//     let task_sections = TaskSection::find_by_branch_id_and_node_id(branch_id, node_id)
//         .execute(data.db_session())
//         .await?
//         .try_collect()
//         .await?;
//
//     let tasks = Task::find_by_branch_id_and_node_id(node_id, branch_id)
//         .execute(data.db_session())
//         .await?
//         .try_collect()
//         .await?;
//
//     Ok(HttpResponse::Ok().json(json!({
//         "taskSections": task_sections,
//         "tasks": tasks,
//     })))
// }
//
// #[post("/task")]
// pub async fn create_task(data: RequestData, task: web::Json<Task>) -> Response {
//     let mut task = task.into_inner();
//
//     AuthNode::auth_update(&data, task.branch_id, task.node_id, task.branch_id).await?;
//
//     task.insert_cb(&data).execute(data.db_session()).await?;
//
//     Ok(HttpResponse::Created().json(task))
// }
//
// #[get("/{branch_id}/{node_id}/{task_id}")]
// pub async fn get_task(data: RequestData, path: web::Path<(Uuid, Uuid, Uuid)>) -> Response {
//     let (branch_id, node_id, task_id) = path.into_inner();
//
//     AuthNode::auth_view(
//         data.db_session(),
//         &OptCurrentUser(Some(data.current_user.clone())),
//         branch_id,
//         node_id,
//         branch_id,
//     )
//     .await?;
//
//     let task = Task::find_by_branch_id_and_node_id_and_id(branch_id, node_id, task_id)
//         .execute(data.db_session())
//         .await?;
//
//     let description = Description::find_by_branch_id_and_object_id(branch_id, task_id)
//         .execute(data.db_session())
//         .await?;
//
//     Ok(HttpResponse::Ok().json(json!({
//         "task": task,
//         "description": description,
//     })))
// }
//
// #[put("/assignees")]
// pub async fn update_assignees(data: RequestData, task: web::Json<UpdateAssigneesTask>) -> Response {
//     let mut task = task.into_inner();
//
//     AuthNode::auth_update(&data, task.branch_id, task.node_id, task.branch_id).await?;
//
//     task.update_cb(&data).execute(data.db_session()).await?;
//
//     Ok(HttpResponse::Ok().json(task))
// }
//
// #[put("/is_completed")]
// pub async fn update_is_completed(data: RequestData, task: web::Json<UpdateIsCompletedTask>) -> Response {
//     let mut task = task.into_inner();
//
//     AuthNode::auth_update(&data, task.branch_id, task.node_id, task.branch_id).await?;
//
//     task.update_cb(&data).execute(data.db_session()).await?;
//
//     Ok(HttpResponse::Ok().json(task))
// }
//
// #[put("/is_archived")]
// pub async fn update_is_archived(data: RequestData, task: web::Json<UpdateIsArchivedTask>) -> Response {
//     let mut task = task.into_inner();
//
//     AuthNode::auth_update(&data, task.branch_id, task.node_id, task.branch_id).await?;
//
//     task.update_cb(&data).execute(data.db_session()).await?;
//
//     Ok(HttpResponse::Ok().json(task))
// }
//
// #[put("/order_index")]
// pub async fn update_order_index(data: RequestData, task: web::Json<UpdateOrderIndexTask>) -> Response {
//     let mut task = task.into_inner();
//
//     AuthNode::auth_update(&data, task.branch_id, task.node_id, task.branch_id).await?;
//
//     task.update_cb(&data).execute(data.db_session()).await?;
//
//     Ok(HttpResponse::Ok().json(task))
// }
