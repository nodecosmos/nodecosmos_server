use crate::api::current_user::OptCurrentUser;
use crate::api::data::RequestData;
use crate::api::types::Response;
use crate::app::App;
use crate::errors::NodecosmosError;
use crate::models::invitation::{Invitation, InvitationContext, InvitationStatus};
use crate::models::traits::Authorization;
use actix_web::{get, post, put, web, HttpResponse};
use charybdis::operations::{Find, InsertWithCallbacks, UpdateWithCallbacks};
use charybdis::types::Uuid;
use serde::Deserialize;
use serde_json::json;

#[get("/{token}/token")]
pub async fn get_invitation_by_token(
    app: web::Data<App>,
    opt_cu: OptCurrentUser,
    token: web::Path<String>,
) -> Response {
    let mut invitation = Invitation::find_by_token(&app.db_session, token.into_inner()).await?;
    let invitee = invitation.invitee(&app.db_session).await?;
    let invite_for_different_user = if let Some(current_user) = opt_cu.0 {
        if let Some(invitee) = &invitee {
            invitee.id != current_user.id
        } else {
            false
        }
    } else {
        false
    };
    let inviter = invitation.inviter(&app.db_session).await?;

    Ok(HttpResponse::Ok().json(json!({
        "invitation": invitation,
        "inviter": inviter,
        "node": invitation.node(&app.db_session).await?,
        "hasUser": invitee.is_some(),
        "inviteForDifferentUser": invite_for_different_user
    })))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InvitationPayload {
    pub node_id: Uuid,
    pub branch_id: Uuid,
}

#[get("/index/{nodeId}/{branchId}")]
pub async fn get_invitations(data: RequestData, payload: web::Path<InvitationPayload>) -> Response {
    let invitations = Invitation::find_by_branch_id_and_node_id(payload.branch_id, payload.node_id)
        .execute(data.db_session())
        .await?
        .try_collect()
        .await?;

    Ok(HttpResponse::Ok().json(invitations))
}

#[post("")]
pub async fn create_invitation(data: RequestData, mut invitation: web::Json<Invitation>) -> Response {
    invitation.auth_creation(&data).await?;

    invitation.insert_cb(&data).execute(data.db_session()).await?;

    Ok(HttpResponse::Ok().json(invitation))
}

#[put("/confirm")]
pub async fn confirm_invitation(data: RequestData, invitation: web::Json<Invitation>) -> Response {
    let mut invitation = invitation.find_by_primary_key().execute(data.db_session()).await?;

    invitation.auth_update(&data).await?;

    let inviter_id = invitation.inviter_id;
    let node = invitation.node(data.db_session()).await?.clone();

    if node.owner_id == Some(inviter_id) || node.editor_ids.as_ref().is_some_and(|ids| ids.contains(&inviter_id)) {
        invitation.ctx = InvitationContext::Confirm;
        invitation.status = InvitationStatus::Accepted.to_string();
        invitation.update_cb(&data).execute(data.db_session()).await?;
    } else {
        return Err(NodecosmosError::Unauthorized(
            "Inviter is no longer an editor of the node.",
        ));
    }

    Ok(HttpResponse::Ok().finish())
}

#[put("/reject")]
pub async fn reject_invitation(data: RequestData, invitation: web::Json<Invitation>) -> Response {
    let mut invitation = invitation.find_by_primary_key().execute(data.db_session()).await?;

    invitation.auth_update(&data).await?;

    invitation.status = InvitationStatus::Rejected.to_string();
    invitation.update_cb(&data).execute(data.db_session()).await?;

    Ok(HttpResponse::Ok().finish())
}
