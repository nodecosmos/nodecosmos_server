use actix_web::{delete, get, post, put, web, HttpResponse};
use charybdis::model::AsNative;
use charybdis::operations::{DeleteWithCallbacks, Find, InsertWithCallbacks, New, UpdateWithCallbacks};
use charybdis::types::Uuid;
use scylla::CachingSession;
use serde_json::json;

use crate::api::data::RequestData;
use crate::api::types::{ActionTypes, Response};
use crate::errors::NodecosmosError;
use crate::models::branch::Branch;
use crate::models::comment::BaseComment;
use crate::models::comment_thread::CommentThread;
use crate::models::contribution_request::{
    BaseContributionRequest, ContributionRequest, UpdateContributionRequestDescription, UpdateContributionRequestTitle,
};
use crate::models::traits::Authorization;
use crate::resources::resource_locker::ResourceLocker;

#[get("/{node_id}")]
pub async fn get_contribution_requests(db_session: web::Data<CachingSession>, node_id: web::Path<Uuid>) -> Response {
    let mut contribution_request = BaseContributionRequest::new();
    contribution_request.node_id = node_id.into_inner();

    let contribution_requests: Vec<BaseContributionRequest> = contribution_request
        .find_by_partition_key()
        .execute(&db_session)
        .await?
        .try_collect()
        .await?;

    Ok(HttpResponse::Ok().json(contribution_requests))
}

#[get("/{nodeId}/{rootId}/{id}")]
pub async fn get_contribution_request(
    db_session: web::Data<CachingSession>,
    contribution_request: web::Path<ContributionRequest>,
) -> Response {
    let contribution_request = contribution_request.find_by_primary_key().execute(&db_session).await?;
    let branch = Branch::find_by_id(contribution_request.id).execute(&db_session).await?;
    let comments = BaseComment::find_by_branch_id(contribution_request.id)
        .execute(&db_session)
        .await?
        .try_collect()
        .await?;
    let threads = CommentThread::find_by_branch_id(contribution_request.id)
        .execute(&db_session)
        .await?
        .try_collect()
        .await?;

    Ok(HttpResponse::Ok().json(json!({
        "contributionRequest": contribution_request,
        "branch": branch,
        "comments": comments,
        "threads": threads,
    })))
}

#[post("")]
pub async fn create_contribution_request(
    data: RequestData,
    mut contribution_request: web::Json<ContributionRequest>,
) -> Response {
    contribution_request.auth_creation(&data).await?;

    contribution_request.insert_cb(&data).execute(data.db_session()).await?;

    Ok(HttpResponse::Ok().json(contribution_request))
}

#[put("/title")]
pub async fn update_contribution_request_title(
    data: RequestData,
    contribution_request: web::Json<UpdateContributionRequestTitle>,
) -> Response {
    let mut native_cr = contribution_request.as_native();

    native_cr.auth_update(&data).await?;

    let mut contribution_request = contribution_request.into_inner();
    contribution_request.update_cb(&None).execute(data.db_session()).await?;

    Ok(HttpResponse::Ok().json(contribution_request))
}

#[put("/description")]
pub async fn update_contribution_request_description(
    data: RequestData,
    mut contribution_request: web::Json<UpdateContributionRequestDescription>,
) -> Response {
    let mut native_cr = contribution_request.as_native();

    native_cr.auth_update(&data).await?;

    contribution_request.update_cb(&None).execute(data.db_session()).await?;

    Ok(HttpResponse::Ok().json(contribution_request))
}

#[delete("/{nodeId}/{rootId}/{id}")]
pub async fn delete_contribution_request(
    data: RequestData,
    contribution_request: web::Path<ContributionRequest>,
) -> Response {
    let mut contribution_request = contribution_request
        .find_by_primary_key()
        .execute(data.db_session())
        .await?;

    contribution_request.auth_update(&data).await?;

    contribution_request.delete_cb(&data).execute(data.db_session()).await?;

    Ok(HttpResponse::Ok().json(contribution_request))
}

#[put("/publish")]
pub async fn publish(data: RequestData, mut contribution_request: web::Json<ContributionRequest>) -> Response {
    contribution_request.auth_update(&data).await?;
    contribution_request.publish(&data).await?;

    Ok(HttpResponse::Ok().finish())
}

#[put("/merge")]
pub async fn merge_contribution_request(
    data: RequestData,
    contribution_request: web::Json<ContributionRequest>,
) -> Response {
    let mut contribution_request = contribution_request
        .find_by_primary_key()
        .execute(data.db_session())
        .await?;
    let branch_id = contribution_request.id;
    let node = contribution_request.node(data.db_session()).await?;
    let root_id = node.root_id;

    node.auth_update(&data).await?;

    // first lock the complete resource to avoid all types of race conditions
    data.resource_locker()
        .lock_resource(root_id, root_id, ResourceLocker::ONE_HOUR)
        .await?;

    // check if merge is allowed in the root
    if let Err(e) = data
        .resource_locker()
        .validate_resource_action_unlocked(ActionTypes::Merge, root_id, root_id, true)
        .await
    {
        // unlock complete resource as merge is not allowed
        data.resource_locker().unlock_resource(root_id, root_id).await?;

        // return reorder not allowed error
        return Err(e);
    }

    // check if merge is allowed in the branch
    if let Err(e) = data
        .resource_locker()
        .validate_resource_action_unlocked(ActionTypes::Merge, root_id, branch_id, true)
        .await
    {
        // unlock complete resource as merge is not allowed
        data.resource_locker().unlock_resource(root_id, root_id).await?;

        // return reorder not allowed error
        return Err(e);
    }

    // execute merge
    let res = contribution_request.merge(&data).await;

    // unlock complete resource
    data.resource_locker().unlock_resource(root_id, root_id).await?;

    match res {
        Ok(_) => Ok(HttpResponse::Ok().finish()),
        Err(e) => match e {
            NodecosmosError::Conflict(e) => {
                let branch = contribution_request.branch(data.db_session()).await?;

                Ok(HttpResponse::Conflict().json(json!({
                    "status": 409,
                    "message": e,
                    "branch": branch,
                })))
            }
            _ => Err(e),
        },
    }
}
