use crate::api::current_user::OptCurrentUser;
use actix_web::{get, put, web, HttpResponse};
use charybdis::types::Uuid;
use scylla::CachingSession;
use serde::Deserialize;

use crate::api::data::RequestData;
use crate::api::types::Response;
use crate::models::branch::update::BranchUpdate;
use crate::models::branch::Branch;
use crate::models::traits::Authorization;

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

#[put("/undo_delete_io")]
pub async fn undo_delete_io(data: RequestData, params: web::Json<BranchPayload>) -> Response {
    let params = params.into_inner();
    let mut branch = Branch::find_by_id(params.branch_id).execute(data.db_session()).await?;

    branch.auth_update(&data).await?;

    let branch = Branch::update(
        data.db_session(),
        params.branch_id,
        BranchUpdate::UndoDeleteIo(params.object_id),
    )
    .await?;

    Ok(HttpResponse::Ok().json(branch))
}
