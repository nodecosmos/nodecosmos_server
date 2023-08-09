use crate::actions::client_session::CurrentUser;
use crate::authorize::{auth_contribution_request_creation, auth_contribution_request_update};
use crate::errors::NodecosmosError;
use crate::models::contribution_request::{
    BaseContributionRequest, ContributionRequest, UpdateContributionRequestDescription,
    UpdateContributionRequestTitle,
};
use crate::models::udts::{Owner, OwnerTypes};
use actix_web::{delete, get, post, put, web, HttpResponse};
use charybdis::{AsNative, Delete, Find, InsertWithCallbacks, New, UpdateWithCallbacks, Uuid};
use scylla::CachingSession;
use serde::Deserialize;

#[get("/{node_id}")]
pub async fn get_contribution_requests(
    db_session: web::Data<CachingSession>,
    node_id: web::Path<Uuid>,
) -> Result<HttpResponse, NodecosmosError> {
    let mut contribution_request = BaseContributionRequest::new();
    contribution_request.node_id = node_id.into_inner();

    let contribution_requests: Vec<BaseContributionRequest> = contribution_request
        .find_by_partition_key(&db_session)
        .await?
        .flatten()
        .collect();

    Ok(HttpResponse::Ok().json(contribution_requests))
}

#[derive(Deserialize)]
pub struct ContributionRequestParams {
    pub node_id: Uuid,
    pub contribution_request_id: Uuid,
}

#[get("/{node_id}/{contribution_request_id}")]
pub async fn get_contribution_request(
    db_session: web::Data<CachingSession>,
    params: web::Path<ContributionRequestParams>,
) -> Result<HttpResponse, NodecosmosError> {
    let params = params.into_inner();

    let mut contribution_request = ContributionRequest::new();
    contribution_request.node_id = params.node_id;
    contribution_request.id = params.contribution_request_id;

    contribution_request
        .find_by_primary_key(&db_session)
        .await?;

    Ok(HttpResponse::Ok().json(contribution_request))
}

#[post("")]
pub async fn create_contribution_request(
    db_session: web::Data<CachingSession>,
    current_user: CurrentUser,
    contribution_request: web::Json<ContributionRequest>,
) -> Result<HttpResponse, NodecosmosError> {
    let mut contribution_request = contribution_request.into_inner();

    auth_contribution_request_creation(&db_session, &contribution_request, &current_user).await?;
    contribution_request.set_owner(Owner {
        id: current_user.id,
        name: current_user.username,
        owner_type: OwnerTypes::User.into(),
        profile_image_url: None,
    });

    contribution_request.insert_cb(&db_session).await?;

    Ok(HttpResponse::Ok().json(contribution_request))
}

#[put("/title")]
pub async fn update_contribution_request_title(
    db_session: web::Data<CachingSession>,
    current_user: CurrentUser,
    contribution_request: web::Json<UpdateContributionRequestTitle>,
) -> Result<HttpResponse, NodecosmosError> {
    let native_cr = contribution_request
        .as_native()
        .find_by_primary_key(&db_session)
        .await?;

    auth_contribution_request_update(&db_session, &native_cr, &current_user).await?;

    let mut contribution_request = contribution_request.into_inner();

    contribution_request.update_cb(&db_session).await?;

    Ok(HttpResponse::Ok().json(contribution_request))
}

#[put("/description")]
pub async fn update_contribution_request_description(
    db_session: web::Data<CachingSession>,
    current_user: CurrentUser,
    contribution_request: web::Json<UpdateContributionRequestDescription>,
) -> Result<HttpResponse, NodecosmosError> {
    let native_cr = contribution_request
        .as_native()
        .find_by_primary_key(&db_session)
        .await?;

    auth_contribution_request_update(&db_session, &native_cr, &current_user).await?;

    let mut contribution_request = contribution_request.into_inner();
    contribution_request.update_cb(&db_session).await?;

    Ok(HttpResponse::Ok().json(contribution_request))
}

#[delete("/{node_id}/{contribution_request_id}")]
pub async fn delete_contribution_request(
    db_session: web::Data<CachingSession>,
    current_user: CurrentUser,
    params: web::Path<ContributionRequestParams>,
) -> Result<HttpResponse, NodecosmosError> {
    let params = params.into_inner();

    let mut contribution_request = ContributionRequest::new();
    contribution_request.node_id = params.node_id;
    contribution_request.id = params.contribution_request_id;

    let contribution_request = contribution_request
        .find_by_primary_key(&db_session)
        .await?;

    auth_contribution_request_update(&db_session, &contribution_request, &current_user).await?;

    contribution_request.delete(&db_session).await?;

    Ok(HttpResponse::Ok().json(contribution_request))
}
