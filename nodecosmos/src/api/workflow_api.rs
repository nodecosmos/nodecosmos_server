use crate::api::data::RequestData;
use crate::api::types::Response;
use crate::models::flow::BaseFlow;
use crate::models::flow_step::FlowStep;
use crate::models::input_output::Io;
use crate::models::traits::Authorization;
use crate::models::workflow::{UpdateInitialInputsWorkflow, UpdateWorkflowTitle, Workflow};
use actix_web::{delete, get, post, put, web, HttpResponse};
use charybdis::model::AsNative;
use charybdis::operations::{DeleteWithCallbacks, Find, InsertWithCallbacks, New, UpdateWithCallbacks};
use charybdis::options::Consistency;
use charybdis::types::Uuid;
use scylla::CachingSession;
use serde::Deserialize;
use serde_json::json;
use tokio_stream::StreamExt;

#[get("/{node_id}")]
pub async fn get_workflow(db_session: web::Data<CachingSession>, node_id: web::Path<Uuid>) -> Response {
    let node_id = node_id.into_inner();

    let workflow = Workflow::find_first_by_partition_key_value(&(node_id,))
        .execute(&db_session)
        .await?;

    // flows
    let mut flow = BaseFlow::new();
    flow.node_id = node_id;
    let flows = flow
        .find_by_partition_key()
        .execute(&db_session)
        .await?
        .try_collect()
        .await?;

    // flow steps
    let mut flow_step = FlowStep::new();
    flow_step.node_id = node_id;
    let flow_steps = flow_step
        .find_by_partition_key()
        .execute(&db_session)
        .await?
        .try_collect()
        .await?;

    // input outputs
    let input_outputs = Io::find_by_partition_key_value(&(workflow.root_node_id,))
        .execute(&db_session)
        .await?
        .try_collect()
        .await?;

    Ok(HttpResponse::Ok().json(json!({
        "workflow": workflow,
        "flows": flows,
        "flowSteps": flow_steps,
        "inputOutputs": input_outputs,
    })))
}

#[post("")]
pub async fn create_workflow(data: RequestData, mut workflow: web::Json<Workflow>) -> Response {
    workflow.auth_creation(&data).await?;

    let input_outputs = Io::find_by_partition_key_value(&(workflow.root_node_id,))
        .execute(data.db_session())
        .await?
        .try_collect()
        .await?;

    let mut existing_workflow = Workflow::find_by_node_id(workflow.node_id)
        .consistency(Consistency::All)
        .execute(data.db_session())
        .await?;

    // ATM we only allow one workflow per node
    // match first existing workflow
    if let Some(existing_workflow) = existing_workflow.next().await {
        return Ok(HttpResponse::Conflict().json(json!({
            "status": 409,
            "message": "Workflow already exists",
            "workflow": existing_workflow?,
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
    workflow.as_native().auth_update(&data).await?;

    workflow.update_cb(&None).execute(data.db_session()).await?;

    Ok(HttpResponse::Ok().json(workflow))
}

#[put("/title")]
pub async fn update_workflow_title(data: RequestData, mut workflow: web::Json<UpdateWorkflowTitle>) -> Response {
    workflow.as_native().auth_update(&data).await?;

    workflow.update_cb(&None).execute(data.db_session()).await?;

    Ok(HttpResponse::Ok().json(workflow))
}

#[derive(Deserialize)]
pub struct DeleteWfParams {
    node_id: Uuid,
    workflow_id: Uuid,
}

#[delete("/{node_id}/{workflow_id}")]
pub async fn delete_workflow(data: RequestData, params: web::Path<DeleteWfParams>) -> Response {
    let mut workflow = Workflow::find_by_node_id_and_id(params.node_id, params.workflow_id)
        .execute(data.db_session())
        .await?;
    workflow.auth_update(&data).await?;

    workflow.delete_cb(&None).execute(data.db_session()).await?;

    Ok(HttpResponse::Ok().json(workflow))
}
