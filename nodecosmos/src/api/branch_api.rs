use crate::api::data::RequestData;
use crate::api::types::Response;
use crate::models::branch::update::BranchUpdate;
use crate::models::branch::Branch;
use crate::models::traits::{Authorization, Reload};
use actix_web::{put, web, HttpResponse};
use charybdis::types::Uuid;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct BranchNodeParams {
    #[serde(rename = "branchId")]
    pub branch_id: Uuid,

    #[serde(rename = "nodeId")]
    pub node_id: Uuid,
}

#[put("/restore_node")]
pub async fn restore_node(data: RequestData, params: web::Json<BranchNodeParams>) -> Response {
    let params = params.into_inner();
    let mut branch = Branch::find_by_id(params.branch_id).execute(data.db_session()).await?;

    branch.auth_update(&data).await?;
    Branch::update(&data, params.branch_id, BranchUpdate::RestoreNode(params.node_id)).await?;

    branch.reload(data.db_session()).await?;

    Ok(HttpResponse::Ok().json(branch))
}

#[put("/undo_delete_node")]
pub async fn undo_delete_node(data: RequestData, params: web::Json<BranchNodeParams>) -> Response {
    let params = params.into_inner();
    let mut branch = Branch::find_by_id(params.branch_id).execute(data.db_session()).await?;

    branch.auth_update(&data).await?;

    Branch::update(
        &data,
        params.branch_id,
        BranchUpdate::UndoDeleteNodes(vec![params.node_id]),
    )
    .await?;

    branch.reload(data.db_session()).await?;

    Ok(HttpResponse::Ok().json(branch))
}

#[derive(Deserialize)]
pub struct BranchIoParams {
    #[serde(rename = "branchId")]
    pub branch_id: Uuid,

    #[serde(rename = "nodeId")]
    pub io_id: Uuid,
}

#[put("/restore_io")]
pub async fn restore_io(data: RequestData, params: web::Json<BranchIoParams>) -> Response {
    let params = params.into_inner();
    let mut branch = Branch::find_by_id(params.branch_id).execute(data.db_session()).await?;

    branch.auth_update(&data).await?;

    Branch::update(&data, params.branch_id, BranchUpdate::RestoreIo(params.io_id)).await?;

    branch.reload(data.db_session()).await?;

    Ok(HttpResponse::Ok().json(branch))
}

#[put("/undo_delete_io")]
pub async fn undo_delete_io(data: RequestData, params: web::Json<BranchIoParams>) -> Response {
    let params = params.into_inner();
    let mut branch = Branch::find_by_id(params.branch_id).execute(data.db_session()).await?;

    branch.auth_update(&data).await?;

    Branch::update(&data, params.branch_id, BranchUpdate::UndoDeleteIo(params.io_id)).await?;

    branch.reload(data.db_session()).await?;

    Ok(HttpResponse::Ok().json(branch))
}

#[derive(Deserialize)]
pub struct BranchFlowParams {
    #[serde(rename = "branchId")]
    pub branch_id: Uuid,

    #[serde(rename = "nodeId")]
    pub flow_id: Uuid,
}

#[put("/restore_flow")]
pub async fn restore_flow(data: RequestData, params: web::Json<BranchFlowParams>) -> Response {
    let params = params.into_inner();
    let mut branch = Branch::find_by_id(params.branch_id).execute(data.db_session()).await?;

    branch.auth_update(&data).await?;

    Branch::update(&data, params.branch_id, BranchUpdate::RestoreFlow(params.flow_id)).await?;

    branch.reload(data.db_session()).await?;

    Ok(HttpResponse::Ok().json(branch))
}

#[put("/undo_delete_flow")]
pub async fn undo_delete_flow(data: RequestData, params: web::Json<BranchFlowParams>) -> Response {
    let params = params.into_inner();
    let mut branch = Branch::find_by_id(params.branch_id).execute(data.db_session()).await?;

    branch.auth_update(&data).await?;

    Branch::update(&data, params.branch_id, BranchUpdate::UndoDeleteFlow(params.flow_id)).await?;

    branch.reload(data.db_session()).await?;

    Ok(HttpResponse::Ok().json(branch))
}

#[derive(Deserialize)]
pub struct BranchFlowStepParams {
    #[serde(rename = "branchId")]
    pub branch_id: Uuid,

    #[serde(rename = "nodeId")]
    pub flow_step_id: Uuid,
}

#[put("/restore_flow_step")]
pub async fn restore_flow_step(data: RequestData, params: web::Json<BranchFlowStepParams>) -> Response {
    let params = params.into_inner();
    let mut branch = Branch::find_by_id(params.branch_id).execute(data.db_session()).await?;

    branch.auth_update(&data).await?;

    Branch::update(
        &data,
        params.branch_id,
        BranchUpdate::RestoreFlowStep(params.flow_step_id),
    )
    .await?;

    branch.reload(data.db_session()).await?;

    Ok(HttpResponse::Ok().json(branch))
}

#[put("/keep_flow_step")]
pub async fn keep_flow_step(data: RequestData, params: web::Json<BranchFlowStepParams>) -> Response {
    let params = params.into_inner();
    let mut branch = Branch::find_by_id(params.branch_id).execute(data.db_session()).await?;

    branch.auth_update(&data).await?;

    Branch::update(&data, params.branch_id, BranchUpdate::KeepFlowStep(params.flow_step_id)).await?;

    branch.reload(data.db_session()).await?;

    Ok(HttpResponse::Ok().json(branch))
}

#[put("/undo_delete_flow_step")]
pub async fn undo_delete_flow_step(data: RequestData, params: web::Json<BranchFlowStepParams>) -> Response {
    let params = params.into_inner();
    let mut branch = Branch::find_by_id(params.branch_id).execute(data.db_session()).await?;

    branch.auth_update(&data).await?;

    Branch::update(
        &data,
        params.branch_id,
        BranchUpdate::UndoDeleteFlowStep(params.flow_step_id),
    )
    .await?;

    branch.reload(data.db_session()).await?;

    Ok(HttpResponse::Ok().json(branch))
}
