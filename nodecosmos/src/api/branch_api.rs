use crate::api::data::RequestData;
use crate::api::types::Response;
use crate::models::branch::update::BranchUpdate;
use crate::models::branch::Branch;
use crate::models::traits::Authorization;
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

    let _ = branch.check_conflicts(data.db_session()).await;

    Ok(HttpResponse::Ok().json(branch))
}

#[put("/undo_delete_node")]
pub async fn undo_delete_node(data: RequestData, params: web::Json<BranchNodeParams>) -> Response {
    let params = params.into_inner();
    let mut branch = Branch::find_by_id(params.branch_id).execute(data.db_session()).await?;

    branch.auth_update(&data).await?;

    Branch::update(&data, params.branch_id, BranchUpdate::UndoDeleteNode(params.node_id)).await?;

    let _ = branch.check_conflicts(data.db_session()).await;

    Ok(HttpResponse::Ok().json(branch))
}
