use crate::api::data::RequestData;
use crate::api::types::Response;
use crate::models::flow::Flow;
use crate::models::flow_step::FlowStep;
use crate::models::io::Io;
use crate::models::node::AuthNode;
use crate::models::workflow::{UpdateInitialInputsWorkflow, UpdateWorkflowTitle, Workflow};
use actix_web::{get, put, web, HttpResponse};
use anyhow::Context;
use charybdis::operations::{Update, UpdateWithCallbacks};
use charybdis::types::Uuid;
use nodecosmos_macros::Branchable;
use scylla::CachingSession;
use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize, Branchable)]
pub struct WorkflowParams {
    #[branch(original_id)]
    pub node_id: Uuid,
    pub branch_id: Uuid,
}

#[get("/{node_id}/{branch_id}")]
pub async fn get_workflow(db_session: web::Data<CachingSession>, params: web::Path<WorkflowParams>) -> Response {
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
    let input_outputs = Io::branched(&db_session, workflow.root_id, &params)
        .await
        .context("Failed to get input outputs")?;

    Ok(HttpResponse::Ok().json(json!({
        "workflow": workflow,
        "flows": flows,
        "flowSteps": flow_steps,
        "inputOutputs": input_outputs,
    })))
}

#[put("/initial_input_ids")]
pub async fn update_initial_inputs(
    data: RequestData,
    mut workflow: web::Json<UpdateInitialInputsWorkflow>,
) -> Response {
    AuthNode::auth_update(&data, workflow.node_id, workflow.branch_id).await?;

    workflow.update_cb(&data).execute(data.db_session()).await?;

    Ok(HttpResponse::Ok().json(workflow))
}

#[put("/title")]
pub async fn update_workflow_title(data: RequestData, workflow: web::Json<UpdateWorkflowTitle>) -> Response {
    AuthNode::auth_update(&data, workflow.node_id, workflow.branch_id).await?;

    workflow.update().execute(data.db_session()).await?;

    Ok(HttpResponse::Ok().json(workflow))
}
