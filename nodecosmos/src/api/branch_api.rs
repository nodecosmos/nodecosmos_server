use crate::api::current_user::OptCurrentUser;
use actix_web::{get, put, web, HttpResponse};
use charybdis::types::Uuid;
use scylla::client::caching_session::CachingSession;
use serde::Deserialize;
use std::collections::HashSet;

use crate::api::data::RequestData;
use crate::api::types::Response;
use crate::models::branch::update::BranchUpdate;
use crate::models::branch::{Branch, GetNodeIdBranch};
use crate::models::traits::Authorization;
use crate::models::user::ShowUser;

#[get("/{id}")]
pub async fn show_branch(
    db_session: web::Data<CachingSession>,
    opt_cu: OptCurrentUser,
    id: web::Path<Uuid>,
) -> Response {
    let mut branch = Branch::find_by_id(id.into_inner()).execute(&db_session).await?;

    // we only reload on actions that require branch/update to be called
    branch.auth_view(&db_session, &opt_cu).await?;

    Ok(HttpResponse::Ok().json(branch))
}

#[get("/{id}/node_id")]
pub async fn get_branch_node_id(db_session: web::Data<CachingSession>, id: web::Path<Uuid>) -> Response {
    let branch = GetNodeIdBranch::find_by_id(id.into_inner())
        .execute(&db_session)
        .await?;

    Ok(HttpResponse::Ok().json(branch))
}

#[derive(Deserialize)]
pub struct BranchPayload {
    #[serde(rename = "branchId")]
    pub branch_id: Uuid,

    #[serde(rename = "objectId")]
    pub object_id: Uuid,
}

#[put("/restore_node")]
pub async fn restore_node(data: RequestData, params: web::Json<BranchPayload>) -> Response {
    let params = params.into_inner();
    let mut branch = Branch::find_by_id(params.branch_id).execute(data.db_session()).await?;

    branch.auth_update(&data).await?;

    let branch = Branch::update(
        data.db_session(),
        params.branch_id,
        BranchUpdate::RestoreNode(params.object_id),
    )
    .await?;

    Ok(HttpResponse::Ok().json(branch))
}

#[put("/undo_delete_node")]
pub async fn undo_delete_node(data: RequestData, params: web::Json<BranchPayload>) -> Response {
    let params = params.into_inner();
    let mut branch = Branch::find_by_id(params.branch_id).execute(data.db_session()).await?;

    branch.auth_update(&data).await?;

    let branch = Branch::update(
        data.db_session(),
        params.branch_id,
        BranchUpdate::UndoDeleteNodes(vec![params.object_id]),
    )
    .await?;

    Ok(HttpResponse::Ok().json(branch))
}

#[put("/restore_flow")]
pub async fn restore_flow(data: RequestData, params: web::Json<BranchPayload>) -> Response {
    let params = params.into_inner();
    let mut branch = Branch::find_by_id(params.branch_id).execute(data.db_session()).await?;

    branch.auth_update(&data).await?;

    let branch = Branch::update(
        data.db_session(),
        params.branch_id,
        BranchUpdate::RestoreFlow(params.object_id),
    )
    .await?;

    Ok(HttpResponse::Ok().json(branch))
}

#[put("/undo_delete_flow")]
pub async fn undo_delete_flow(data: RequestData, params: web::Json<BranchPayload>) -> Response {
    let params = params.into_inner();
    let mut branch = Branch::find_by_id(params.branch_id).execute(data.db_session()).await?;

    branch.auth_update(&data).await?;

    let branch = Branch::update(
        data.db_session(),
        params.branch_id,
        BranchUpdate::UndoDeleteFlow(params.object_id),
    )
    .await?;

    Ok(HttpResponse::Ok().json(branch))
}

#[put("/restore_flow_step")]
pub async fn restore_flow_step(data: RequestData, params: web::Json<BranchPayload>) -> Response {
    let params = params.into_inner();
    let mut branch = Branch::find_by_id(params.branch_id).execute(data.db_session()).await?;

    branch.auth_update(&data).await?;

    let branch = Branch::update(
        data.db_session(),
        params.branch_id,
        BranchUpdate::RestoreFlowStep(params.object_id),
    )
    .await?;

    Ok(HttpResponse::Ok().json(branch))
}

#[put("/keep_flow_step")]
pub async fn keep_flow_step(data: RequestData, params: web::Json<BranchPayload>) -> Response {
    let params = params.into_inner();
    let mut branch = Branch::find_by_id(params.branch_id).execute(data.db_session()).await?;

    branch.auth_update(&data).await?;

    let branch = Branch::update(
        data.db_session(),
        params.branch_id,
        BranchUpdate::KeepFlowStep(params.object_id),
    )
    .await?;

    Ok(HttpResponse::Ok().json(branch))
}

#[put("/undo_delete_flow_step")]
pub async fn undo_delete_flow_step(data: RequestData, params: web::Json<BranchPayload>) -> Response {
    let params = params.into_inner();
    let mut branch = Branch::find_by_id(params.branch_id).execute(data.db_session()).await?;

    branch.auth_update(&data).await?;

    let branch = Branch::update(
        data.db_session(),
        params.branch_id,
        BranchUpdate::UndoDeleteFlowStep(params.object_id),
    )
    .await?;

    Ok(HttpResponse::Ok().json(branch))
}

#[put("/restore_io")]
pub async fn restore_io(data: RequestData, params: web::Json<BranchPayload>) -> Response {
    let params = params.into_inner();
    let mut branch = Branch::find_by_id(params.branch_id).execute(data.db_session()).await?;

    branch.auth_update(&data).await?;

    let branch = Branch::update(
        data.db_session(),
        params.branch_id,
        BranchUpdate::RestoreIo(params.object_id),
    )
    .await?;

    Ok(HttpResponse::Ok().json(branch))
}

#[put("/undo_delete_flow_step_io/{branch_id}/{fs_id}/{fs_node_id}/{io_id}")]
pub async fn undo_delete_flow_step_io(data: RequestData, params: web::Path<(Uuid, Uuid, Uuid, Uuid)>) -> Response {
    let (branch_id, fs_id, fs_node_id, io_id) = params.into_inner();
    let mut branch = Branch::find_by_id(branch_id).execute(data.db_session()).await?;

    branch.auth_update(&data).await?;

    Branch::update(data.db_session(), branch_id, BranchUpdate::UndoDeleteIo(io_id)).await?;

    branch = Branch::update(
        data.db_session(),
        branch_id,
        BranchUpdate::UndoDeleteOutput((fs_id, fs_node_id, io_id)),
    )
    .await?;

    Ok(HttpResponse::Ok().json(branch))
}

#[put("/undo_delete_initial_io/{branch_id}/{io_id}")]
pub async fn undo_delete_initial_io(data: RequestData, params: web::Path<(Uuid, Uuid)>) -> Response {
    let (branch_id, io_id) = params.into_inner();
    let mut branch = Branch::find_by_id(branch_id).execute(data.db_session()).await?;

    branch.auth_update(&data).await?;

    Branch::update(data.db_session(), branch_id, BranchUpdate::UndoDeleteIo(io_id)).await?;

    let mut set = HashSet::new();
    set.insert(io_id);

    branch = Branch::update(
        data.db_session(),
        branch_id,
        BranchUpdate::UndoDeleteWorkflowInitialInputs(set),
    )
    .await?;

    Ok(HttpResponse::Ok().json(branch))
}

#[get("/editors/{branchId}")]
pub async fn get_branch_editors(db_session: web::Data<CachingSession>, pk: web::Path<Uuid>) -> Response {
    let branch = Branch::find_by_id(pk.into_inner()).execute(&db_session).await?;
    let user_ids = branch.editor_ids.unwrap_or_default();
    let users = ShowUser::find_by_ids(&db_session, &user_ids).await?;

    Ok(HttpResponse::Ok().json(users))
}
