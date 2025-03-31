use actix_web::{get, put, web, HttpResponse};
use anyhow::Context;
use charybdis::operations::Update;
use charybdis::types::Uuid;
use scylla::client::caching_session::CachingSession;
use serde_json::json;

use crate::api::current_user::OptCurrentUser;
use crate::api::data::RequestData;
use crate::api::types::Response;
use crate::models::flow::{Flow, TitleFlow};
use crate::models::flow_step::{FlowStep, PkFlowStep};
use crate::models::io::{Io, TitleIo};
use crate::models::node::AuthNode;
use crate::models::traits::NodeBranchParams;
use crate::models::workflow::{UpdateWorkflowTitle, Workflow};

#[get("/{root_id}/{branch_id}/{node_id}")]
pub async fn get_workflow(
    db_session: web::Data<CachingSession>,
    opt_cu: OptCurrentUser,
    params: web::Path<NodeBranchParams>,
) -> Response {
    AuthNode::auth_view(&db_session, &opt_cu, params.branch_id, params.node_id, params.root_id).await?;

    let params = params.into_inner();

    let workflow = Workflow::branched(&db_session, &params)
        .await
        .context("Failed to get workflow")?;

    let flows = Flow::branched(&db_session, &params)
        .await
        .context("Failed to get flows")?;

    let flow_steps = FlowStep::branched(&db_session, &params)
        .await
        .context("Failed to get flow steps")?;

    let input_outputs = Io::branched(&db_session, &params)
        .await
        .context("Failed to get input outputs")?;

    Ok(HttpResponse::Ok().json(json!({
        "workflow": workflow,
        "flows": flows,
        "flowSteps": flow_steps,
        "inputOutputs": input_outputs,
    })))
}

#[get("/index/branch_data/{branch_id}/{node_id}/{root_id}")]
pub async fn get_workflow_branch_commit_data(
    db_session: web::Data<CachingSession>,
    opt_cu: OptCurrentUser,
    params: web::Path<(Uuid, Uuid, Uuid)>,
) -> Response {
    let params = params.into_inner();
    let (branch_id, node_id, root_id) = (params.0, params.1, params.2);

    AuthNode::auth_view(&db_session, &opt_cu, branch_id, node_id, root_id).await?;

    let flows = TitleFlow::find_by_branch_id(branch_id)
        .execute(&db_session)
        .await?
        .try_collect()
        .await?;

    let flow_steps = PkFlowStep::find_by_branch_id(branch_id)
        .execute(&db_session)
        .await?
        .try_collect()
        .await?;

    let input_outputs = TitleIo::find_by_branch_id(branch_id)
        .execute(&db_session)
        .await?
        .try_collect()
        .await?;

    Ok(HttpResponse::Ok().json(json!({
        "flows": flows,
        "flowSteps": flow_steps,
        "inputOutputs": input_outputs,
    })))
}

#[put("/title")]
pub async fn update_workflow_title(data: RequestData, workflow: web::Json<UpdateWorkflowTitle>) -> Response {
    AuthNode::auth_update(&data, workflow.branch_id, workflow.node_id, workflow.root_id).await?;

    workflow.update().execute(data.db_session()).await?;

    Ok(HttpResponse::Ok().json(workflow))
}
