use crate::api::current_user::OptCurrentUser;
use crate::api::data::RequestData;
use crate::api::types::Response;
use crate::models::description::Description;
use crate::models::node::AuthNode;
use crate::models::task::{Task, UpdateAssigneesTask, UpdatePositionTask, UpdateTitleTask};
use crate::models::task_section::{TaskSection, UpdateOrderIndexTaskSection, UpdateTitleTaskSection};
use actix_web::{get, post, put, web, HttpResponse};
use charybdis::operations::{InsertWithCallbacks, UpdateWithCallbacks};
use charybdis::types::Uuid;
use serde_json::json;

#[post("/sections")]
pub async fn create_task_section(data: RequestData, task_section: web::Json<TaskSection>) -> Response {
    let mut task_section = task_section.into_inner();

    AuthNode::auth_update(
        &data,
        task_section.branch_id,
        task_section.node_id,
        task_section.branch_id,
    )
    .await?;

    task_section.insert_cb(&None).execute(data.db_session()).await?;

    Ok(HttpResponse::Created().json(task_section))
}

#[put("/section_order_index")]
pub async fn update_section_order_index(
    data: RequestData,
    task_section: web::Json<UpdateOrderIndexTaskSection>,
) -> Response {
    let mut task_section = task_section.into_inner();

    AuthNode::auth_update(
        &data,
        task_section.branch_id,
        task_section.node_id,
        task_section.branch_id,
    )
    .await?;

    task_section.update_cb(&None).execute(data.db_session()).await?;

    Ok(HttpResponse::Ok().json(task_section))
}

#[put("/section_title")]
pub async fn update_section_title(data: RequestData, task_section: web::Json<UpdateTitleTaskSection>) -> Response {
    let mut task_section = task_section.into_inner();

    AuthNode::auth_update(
        &data,
        task_section.branch_id,
        task_section.node_id,
        task_section.branch_id,
    )
    .await?;

    task_section.update_cb(&None).execute(data.db_session()).await?;

    Ok(HttpResponse::Ok().json(task_section))
}

#[get("/{branch_id}/{node_id}")]
pub async fn get_node_tasks(data: RequestData, path: web::Path<(Uuid, Uuid)>) -> Response {
    let (branch_id, node_id) = path.into_inner();

    AuthNode::auth_view(
        data.db_session(),
        &OptCurrentUser(Some(data.current_user.clone())),
        branch_id,
        node_id,
        branch_id,
    )
    .await?;

    let task_sections = TaskSection::find_by_branch_id_and_node_id(branch_id, node_id)
        .execute(data.db_session())
        .await?
        .try_collect()
        .await?;

    let tasks = Task::find_by_branch_id_and_node_id(node_id, branch_id)
        .execute(data.db_session())
        .await?
        .try_collect()
        .await?;

    Ok(HttpResponse::Ok().json(json!({
        "sections": task_sections,
        "tasks": tasks,
    })))
}

#[post("/task")]
pub async fn create_task(data: RequestData, task: web::Json<Task>) -> Response {
    let mut task = task.into_inner();

    AuthNode::auth_update(&data, task.branch_id, task.node_id, task.branch_id).await?;

    task.insert_cb(&data).execute(data.db_session()).await?;

    Ok(HttpResponse::Created().json(task))
}

#[get("/{branch_id}/{node_id}/{task_id}")]
pub async fn get_task(data: RequestData, path: web::Path<(Uuid, Uuid, Uuid)>) -> Response {
    let (branch_id, node_id, task_id) = path.into_inner();

    AuthNode::auth_view(
        data.db_session(),
        &OptCurrentUser(Some(data.current_user.clone())),
        branch_id,
        node_id,
        branch_id,
    )
    .await?;

    let task = Task::find_by_branch_id_and_node_id_and_id(branch_id, node_id, task_id)
        .execute(data.db_session())
        .await?;

    let description = Description::find_by_branch_id_and_object_id(branch_id, task_id)
        .execute(data.db_session())
        .await?;

    Ok(HttpResponse::Ok().json(json!({
        "task": task,
        "description": description,
    })))
}

#[put("/task_title")]
pub async fn update_task_title(data: RequestData, task: web::Json<UpdateTitleTask>) -> Response {
    let mut task = task.into_inner();

    AuthNode::auth_update(&data, task.branch_id, task.node_id, task.branch_id).await?;

    task.update_cb(&None).execute(data.db_session()).await?;

    Ok(HttpResponse::Ok().json(task))
}

#[put("/assignees")]
pub async fn update_assignees(data: RequestData, task: web::Json<UpdateAssigneesTask>) -> Response {
    let mut task = task.into_inner();

    AuthNode::auth_update(&data, task.branch_id, task.node_id, task.branch_id).await?;

    task.update_cb(&data).execute(data.db_session()).await?;

    Ok(HttpResponse::Ok().json(task))
}

#[put("/task_position")]
pub async fn update_task_position(data: RequestData, task: web::Json<UpdatePositionTask>) -> Response {
    let mut task = task.into_inner();

    AuthNode::auth_update(&data, task.branch_id, task.node_id, task.branch_id).await?;

    task.update_cb(&None).execute(data.db_session()).await?;

    Ok(HttpResponse::Ok().json(task))
}
