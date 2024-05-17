use crate::api::data::RequestData;
use crate::api::types::Response;
use crate::models::invitation::Invitation;
use crate::models::traits::Authorization;
use actix_web::{web, HttpResponse};

pub async fn create_invitation(data: RequestData, mut invitation: web::Json<Invitation>) -> Response {
    invitation.auth_creation(&data).await?;

    invitation.insert_cb(&data).execute(data.db_session()).await?;

    Ok(HttpResponse::Ok().json(invitation))
}
