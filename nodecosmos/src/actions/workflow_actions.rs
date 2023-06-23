use crate::actions::client_session::CurrentUser;
use crate::authorize::auth_workflow_creation;
use crate::errors::NodecosmosError;
use crate::models::flow::BaseFlow;
use crate::models::flow_step::FlowStep;
use crate::models::input_output::{find_input_output_query, InputOutput};
use crate::models::workflow::{find_workflow_query, UpdateInitialInputsWorkflow, Workflow};

use actix_web::{get, post, put, web, HttpResponse};
use charybdis::{Find, InsertWithCallbacks, New, UpdateWithCallbacks, Uuid};
use scylla::CachingSession;
use serde_json::json;

#[get("/{node_id}")]
pub async fn get_workflow(
    db_session: web::Data<CachingSession>,
    node_id: web::Path<Uuid>,
) -> Result<HttpResponse, NodecosmosError> {
    let node_id = node_id.into_inner();

    // Currently we only support one workflow per node, in future we will support multiple
    // workflows per node.
    let workflow =
        Workflow::find_one(&db_session, find_workflow_query!("node_id = ?"), (node_id,)).await?;

    let mut flow = BaseFlow::new();
    flow.node_id = node_id;
    flow.workflow_id = workflow.id;

    let mut flows = flow.find_by_partition_key(&db_session).await?;

    let mut flow_step = FlowStep::new();
    flow_step.node_id = node_id;
    flow_step.workflow_id = workflow.id;

    let mut flow_steps = flow_step.find_by_partition_key(&db_session).await?;

    let mut input_output_ids = vec![];
    let mut flows_vec = vec![];
    let mut flow_steps_vec = vec![];
    let mut input_outputs = vec![];

    if let Some(initial_input_ids) = &workflow.initial_input_ids {
        input_output_ids.extend(initial_input_ids.iter().cloned());
    }

    while let Some(flow) = flows.next() {
        if let Ok(flow) = flow {
            flows_vec.push(flow);
        }
    }

    while let Some(step) = flow_steps.next() {
        if let Ok(step) = step {
            step.input_ids_by_node_id.iter().for_each(|io| {
                io.values().for_each(|ids| {
                    input_output_ids.extend(ids);
                });
            });
            step.output_ids_by_node_id.iter().for_each(|io| {
                io.values().for_each(|ids| {
                    input_output_ids.extend(ids);
                });
            });

            flow_steps_vec.push(step);
        }
    }

    if !input_output_ids.is_empty() {
        let mut input_outputs_res = InputOutput::find(
            &db_session,
            find_input_output_query!("node_id = ? AND workflow_id = ? AND id IN ?"),
            (workflow.node_id, workflow.id, input_output_ids),
        )
        .await?;

        while let Some(input_output) = input_outputs_res.next() {
            if let Ok(input_output) = input_output {
                input_outputs.push(input_output);
            }
        }
    }

    Ok(HttpResponse::Ok().json(json!({
        "workflow": workflow,
        "flows": flows_vec,
        "flowSteps": flow_steps_vec,
        "inputOutputs": input_outputs,
    })))
}

#[post("")]
pub async fn create_workflow(
    db_session: web::Data<CachingSession>,
    current_user: CurrentUser,
    workflow: web::Json<Workflow>,
) -> Result<HttpResponse, NodecosmosError> {
    let mut workflow = workflow.into_inner();

    auth_workflow_creation(&db_session, workflow.node_id, current_user).await?;

    workflow.insert_cb(&db_session).await?;

    Ok(HttpResponse::Ok().json(json!({
        "success": true,
        "workflow": workflow,
    })))
}

#[put("/initial_input_ids")]
pub async fn update_initial_inputs(
    db_session: web::Data<CachingSession>,
    current_user: CurrentUser,
    workflow: web::Json<UpdateInitialInputsWorkflow>,
) -> Result<HttpResponse, NodecosmosError> {
    let mut workflow = workflow.into_inner();

    auth_workflow_creation(&db_session, workflow.node_id, current_user).await?;

    workflow.update_cb(&db_session).await?;

    Ok(HttpResponse::Ok().json(json!({
        "success": true,
        "workflow": workflow,
    })))
}
