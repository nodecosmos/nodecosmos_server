use crate::api::data::RequestData;
use crate::api::types::Response;
use crate::models::flow::Flow;
use crate::models::flow_step::FlowStep;
use crate::models::input_output::Io;
use crate::models::node::AuthNode;
use crate::models::traits::Branchable;
use crate::models::workflow::{UpdateInitialInputsWorkflow, UpdateWorkflowTitle, Workflow};
use actix_web::{get, post, put, web, HttpResponse};
use charybdis::operations::{InsertWithCallbacks, Update, UpdateWithCallbacks};
use charybdis::options::Consistency;
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

    let workflow = Workflow::branched(&db_session, &params).await?;
    let flows = Flow::branched(&db_session, &params).await?;
    let flow_steps = FlowStep::branched(&db_session, &params).await?;
    let input_outputs = Io::branched(&db_session, workflow.root_id, &params).await?;

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

    let existing_workflow =
        Workflow::maybe_find_first_by_node_id_and_branch_id(workflow.node_id, workflow.original_id())
            .consistency(Consistency::All)
            .execute(data.db_session())
            .await?;

    // ATM we only allow one workflow per node
    // match first existing workflow
    if let Some(existing_workflow) = existing_workflow {
        let input_outputs = existing_workflow.input_outputs(data.db_session()).await?;

        return Ok(HttpResponse::Conflict().json(json!({
            "status": 409,
            "message": "Workflow already exists",
            "workflow": existing_workflow,
            "inputOutputs": input_outputs,
        })));
    }

    workflow.insert_cb(&data).execute(data.db_session()).await?;

    let input_outputs = workflow.input_outputs(data.db_session()).await?;

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

    workflow.update_cb(&data).execute(data.db_session()).await?;

    Ok(HttpResponse::Ok().json(workflow))
}

#[put("/title")]
pub async fn update_workflow_title(data: RequestData, workflow: web::Json<UpdateWorkflowTitle>) -> Response {
    AuthNode::auth_update(&data, workflow.node_id, workflow.branch_id).await?;

    workflow.update().execute(data.db_session()).await?;

    Ok(HttpResponse::Ok().json(workflow))
}
