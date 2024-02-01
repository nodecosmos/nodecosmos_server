use crate::api::authorization::Authorization;
use crate::api::data::RequestData;
use crate::api::types::Response;
use crate::models::flow::BaseFlow;
use crate::models::flow_step::FlowStep;
use crate::models::input_output::Io;
use crate::models::workflow::{UpdateInitialInputsWorkflow, UpdateWorkflowTitle, Workflow};
use actix_web::{delete, get, post, put, web, HttpResponse};
use charybdis::model::AsNative;
use charybdis::operations::{DeleteWithCallbacks, Find, InsertWithCallbacks, New, UpdateWithCallbacks};
use charybdis::types::Uuid;
use scylla::CachingSession;
use serde::Deserialize;
use serde_json::json;

#[get("/{node_id}")]
pub async fn get_workflow(db_session: web::Data<CachingSession>, node_id: web::Path<Uuid>) -> Response {
    let node_id = node_id.into_inner();

    let workflow = Workflow::find_first_by_partition_key_value(&db_session, (node_id,)).await?;

    // flows
    let mut flow = BaseFlow::new();
    flow.node_id = node_id;
    let flows = flow.find_by_partition_key(&db_session).await?.try_collect().await?;

    // flow steps
    let mut flow_step = FlowStep::new();
    flow_step.node_id = node_id;
    let flow_steps = flow_step
        .find_by_partition_key(&db_session)
        .await?
        .try_collect()
        .await?;

    // input outputs
    let input_outputs = Io::find_by_partition_key_value(&db_session, (workflow.root_node_id,))
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

    workflow.insert_cb(data.db_session()).await?;

    let input_outputs = Io::find_by_partition_key_value(data.db_session(), (workflow.root_node_id,))
        .await?
        .try_collect()
        .await?;

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

    workflow.update_cb(data.db_session()).await?;

    Ok(HttpResponse::Ok().json(workflow))
}

#[put("/title")]
pub async fn update_workflow_title(data: RequestData, mut workflow: web::Json<UpdateWorkflowTitle>) -> Response {
    workflow.as_native().auth_update(&data).await?;

    workflow.update_cb(data.db_session()).await?;

    Ok(HttpResponse::Ok().json(workflow))
}

#[derive(Deserialize)]
pub struct DeleteWfParams {
    node_id: Uuid,
    workflow_id: Uuid,
}

#[delete("/{node_id}/{workflow_id}")]
pub async fn delete_workflow(data: RequestData, params: web::Path<DeleteWfParams>) -> Response {
    let mut workflow = Workflow::find_by_node_id_and_id(data.db_session(), params.node_id, params.workflow_id).await?;
    workflow.auth_update(&data).await?;

    workflow.delete_cb(data.db_session()).await?;

    Ok(HttpResponse::Ok().json(workflow))
}
