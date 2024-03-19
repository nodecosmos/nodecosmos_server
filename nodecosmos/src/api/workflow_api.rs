use crate::api::data::RequestData;
use crate::api::types::Response;
use crate::models::flow::BaseFlow;
use crate::models::flow_step::FlowStep;
use crate::models::input_output::Io;
use crate::models::node::AuthNode;
use crate::models::traits::Branchable;
use crate::models::workflow::{UpdateInitialInputsWorkflow, UpdateWorkflowTitle, Workflow};
use actix_web::{delete, get, post, put, web, HttpResponse};
use charybdis::operations::{DeleteWithCallbacks, InsertWithCallbacks, UpdateWithCallbacks};
use charybdis::options::Consistency;
use charybdis::types::Uuid;
use scylla::CachingSession;
use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize)]
pub struct WorkflowParams {
    pub node_id: Uuid,
    pub branch_id: Uuid,
}

#[get("/{node_id}/{branch_id}")]
pub async fn get_workflow(db_session: web::Data<CachingSession>, params: web::Path<WorkflowParams>) -> Response {
    let params = params.into_inner();

    let workflow = Workflow::find_first_by_node_id_and_branch_id(params.node_id, params.branch_id)
        .execute(&db_session)
        .await?;

    // flows
    let flows = BaseFlow::branched(&db_session, &params).await?;

    // flow steps
    let flow_steps = FlowStep::branched(&db_session, &params).await?;

    // input outputs
    let input_outputs = Io::branched(&db_session, workflow.root_node_id, &params).await?;

    Ok(HttpResponse::Ok().json(json!({
        "workflow": workflow,
        "flows": flows,
        "flowSteps": flow_steps,
        "inputOutputs": input_outputs,
    })))
}

#[post("")]
pub async fn create_workflow(data: RequestData, mut workflow: web::Json<Workflow>) -> Response {
    AuthNode::auth_update(&data, workflow.node_id, workflow.branch_id).await?;

    let input_outputs =
        Io::find_by_root_node_id_and_branch_id(workflow.root_node_id, workflow.branchise_id(workflow.root_node_id))
            .execute(data.db_session())
            .await?
            .try_collect()
            .await?;

    let existing_workflow = Workflow::maybe_find_first_by_node_id(workflow.node_id)
        .consistency(Consistency::All)
        .execute(data.db_session())
        .await?;

    // ATM we only allow one workflow per node
    // match first existing workflow
    if let Some(existing_workflow) = existing_workflow {
        return Ok(HttpResponse::Conflict().json(json!({
            "status": 409,
            "message": "Workflow already exists",
            "workflow": existing_workflow,
            "inputOutputs": input_outputs,
        })));
    }

    workflow.insert_cb(&None).execute(data.db_session()).await?;

    Ok(HttpResponse::Ok().json(json!({
        "workflow": workflow,
        "inputOutputs": input_outputs,
    })))
}

#[put("/initial_input_ids")]
pub async fn update_initial_inputs(
    data: RequestData,
    mut workflow: web::Json<UpdateInitialInputsWorkflow>,
) -> Response {
    AuthNode::auth_update(&data, workflow.node_id, workflow.branch_id).await?;

    workflow.update_cb(&None).execute(data.db_session()).await?;

    Ok(HttpResponse::Ok().json(workflow))
}

#[put("/title")]
pub async fn update_workflow_title(data: RequestData, mut workflow: web::Json<UpdateWorkflowTitle>) -> Response {
    AuthNode::auth_update(&data, workflow.node_id, workflow.branch_id).await?;

    workflow.update_cb(&None).execute(data.db_session()).await?;

    Ok(HttpResponse::Ok().json(workflow))
}

#[derive(Deserialize)]
pub struct DeleteWfParams {
    node_id: Uuid,
    branch_id: Uuid,
    workflow_id: Uuid,
}

#[delete("/{node_id}/{branch_id}/{workflow_id}")]
pub async fn delete_workflow(data: RequestData, params: web::Path<DeleteWfParams>) -> Response {
    AuthNode::auth_update(&data, params.node_id, params.branch_id).await?;

    let mut workflow =
        Workflow::find_by_node_id_and_branch_id_and_id(params.node_id, params.branch_id, params.workflow_id)
            .execute(data.db_session())
            .await?;

    workflow.delete_cb(&None).execute(data.db_session()).await?;

    Ok(HttpResponse::Ok().json(workflow))
}
