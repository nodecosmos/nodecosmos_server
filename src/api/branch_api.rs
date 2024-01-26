use crate::api::data::RequestData;
use crate::api::types::Response;
use crate::models::authorization::Authorization;
use crate::models::branch::update::BranchUpdate;
use crate::models::branch::Branch;
use actix_web::{put, web, HttpResponse};
use charybdis::operations::Find;
use charybdis::types::Uuid;
use scylla::CachingSession;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct RestoreNodeParams {
    #[serde(rename = "branchId")]
    pub branch_id: Uuid,

    #[serde(rename = "nodeId")]
    pub node_id: Uuid,
}

#[put("/restore_node")]
pub async fn restore_node(data: RequestData, params: web::Json<RestoreNodeParams>) -> Response {
    let params = params.into_inner();
    let mut branch = Branch::find_by_id(data.db_session(), params.branch_id).await?;

    branch.auth_update(&data).await?;

    Branch::update(
        data.db_session(),
        params.branch_id,
        BranchUpdate::RestoreNode(params.node_id),
    )
    .await;

    Ok(HttpResponse::Ok().json(branch))
}
