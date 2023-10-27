use crate::authorize::auth_commit;
use crate::errors::NodecosmosError;
use crate::models::commit::node_commit::NodeCommit;
use crate::models::commit::types::{CommitObjectTypes, CommitTypes, Committable};
use crate::models::commit::workflow_commit::WorkflowCommit;
use crate::models::commit::Commit;
use crate::models::node::{Node, UpdateDescriptionNode, UpdateTitleNode};
use crate::models::user::CurrentUser;
use crate::models::workflow::Workflow;
use actix_web::{post, web, HttpResponse};
use charybdis::types::Uuid;
use scylla::CachingSession;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct CommitParams {
    pub contribution_request_id: Uuid,
    pub node_id: Uuid,
    pub object_id: Option<Uuid>,
}

#[post("/nodes/{contribution_request_id}/{node_id}/create")]
pub async fn create_node_commit(
    db_session: web::Data<CachingSession>,
    node: web::Json<Node>,
    current_user: CurrentUser,
    params: web::Path<CommitParams>,
) -> Result<HttpResponse, NodecosmosError> {
    auth_commit(&db_session, &params, &current_user).await?;

    <Commit as NodeCommit>::create_node_commit(&db_session, params.into_inner(), current_user.id, &node).await?;

    Ok(HttpResponse::Ok().json(node))
}

#[post("/nodes/{contribution_request_id}/{node_id}/title")]
pub async fn update_node_commit_title(
    db_session: web::Data<CachingSession>,
    node: web::Json<UpdateTitleNode>,
    current_user: CurrentUser,
    params: web::Path<CommitParams>,
) -> Result<HttpResponse, NodecosmosError> {
    auth_commit(&db_session, &params, &current_user).await?;

    let node = node.into_inner();

    Commit::create_update_object_commit(
        &db_session,
        params.into_inner(),
        current_user.id,
        node.id,
        "title",
        node.title.clone(),
        CommitTypes::Update(CommitObjectTypes::Node(Committable::Title)),
    )
    .await?;

    Ok(HttpResponse::Ok().json(node))
}

#[post("/nodes/{contribution_request_id}/{node_id}/description")]
pub async fn update_node_commit_description(
    db_session: web::Data<CachingSession>,
    node: web::Json<UpdateDescriptionNode>,
    current_user: CurrentUser,
    params: web::Path<CommitParams>,
) -> Result<HttpResponse, NodecosmosError> {
    auth_commit(&db_session, &params, &current_user).await?;

    let node = node.into_inner();

    Commit::create_update_object_commit(
        &db_session,
        params.into_inner(),
        current_user.id,
        node.id,
        "description",
        node.description.clone().unwrap_or_default(),
        CommitTypes::Update(CommitObjectTypes::Node(Committable::Description)),
    )
    .await?;

    Ok(HttpResponse::Ok().json(node))
}

#[post("/nodes/{contribution_request_id}/{node_id}/{object_id}/delete")]
pub async fn delete_node_commit(
    db_session: web::Data<CachingSession>,
    current_user: CurrentUser,
    params: web::Path<CommitParams>,
) -> Result<HttpResponse, NodecosmosError> {
    auth_commit(&db_session, &params, &current_user).await?;

    if let Some(object_id) = params.object_id {
        Commit::create_delete_object_commit(
            &db_session,
            params.into_inner(),
            current_user.id,
            object_id,
            CommitTypes::Delete(CommitObjectTypes::Node(Committable::BaseObject)),
        )
        .await?;

        Ok(HttpResponse::Ok().finish())
    } else {
        Ok(HttpResponse::BadRequest().finish())
    }
}

#[post("/workflows/{contribution_request_id}/{node_id}/create")]
pub async fn create_workflow_commit(
    db_session: web::Data<CachingSession>,
    workflow: web::Json<Workflow>,
    current_user: CurrentUser,
    params: web::Path<CommitParams>,
) -> Result<HttpResponse, NodecosmosError> {
    auth_commit(&db_session, &params, &current_user).await?;

    <Commit as WorkflowCommit>::create_workflow_commit(&db_session, params.into_inner(), current_user.id, &workflow)
        .await?;

    Ok(HttpResponse::Ok().json(workflow))
}
