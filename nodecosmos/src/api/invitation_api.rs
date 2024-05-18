use crate::api::current_user::OptCurrentUser;
use crate::api::data::RequestData;
use crate::api::types::Response;
use crate::app::App;
use crate::errors::NodecosmosError;
use crate::models::invitation::{Invitation, InvitationContext, InvitationStatus};
use crate::models::traits::Authorization;
use crate::models::user::EmailUser;
use actix_web::{get, post, web, HttpResponse};
use charybdis::operations::{Find, InsertWithCallbacks, UpdateWithCallbacks};
use serde_json::json;

#[post("")]
pub async fn create_invitation(data: RequestData, mut invitation: web::Json<Invitation>) -> Response {
    invitation.auth_creation(&data).await?;

    invitation.insert_cb(&data).execute(data.db_session()).await?;

    Ok(HttpResponse::Ok().json(invitation))
}

#[post("/confirm")]
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

#[get("/{token}/token")]
pub async fn find_invitation_by_token(
    app: web::Data<App>,
    opt_cu: OptCurrentUser,
    token: web::Path<String>,
) -> Response {
    let invitation = Invitation::find_by_token(&app.db_session, token.into_inner()).await?;
    let user = EmailUser::maybe_find_first_by_email(invitation.email.clone())
        .execute(&app.db_session)
        .await?;
    let invite_for_different_email = if let Some(user) = opt_cu.0 {
        user.email != invitation.email
    } else {
        false
    };

    Ok(HttpResponse::Ok().json(json!({
        "invitation": invitation,
        "hasUser": user.is_some(),
        "inviteForDifferentEmail": invite_for_different_email
    })))
}
