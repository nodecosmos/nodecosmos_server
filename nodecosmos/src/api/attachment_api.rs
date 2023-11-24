use crate::api::authorization::auth_node_update_by_id;
use crate::api::types::Response;
use crate::app::App;
use crate::models::attachment::Attachment;
use crate::models::user::CurrentUser;
use crate::services::aws::s3::get_s3_presigned_url;
use actix_multipart::Multipart;
use actix_web::{get, post, web, HttpResponse};
use charybdis::operations::InsertWithCallbacks;
use charybdis::types::Uuid;
use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize)]
pub struct ImageAttachmentParams {
    pub node_id: Uuid,
    pub object_id: Uuid,
}

#[post("/{node_id}/{object_id}/upload_image")]
pub async fn upload_image(
    params: web::Path<ImageAttachmentParams>,
    app: web::Data<App>,
    payload: Multipart,
    current_user: CurrentUser,
) -> Response {
    auth_node_update_by_id(&params.node_id, &app.db_session, &current_user).await?;

    let attachment = Attachment::create_image(&params, &app, &current_user, payload).await?;

    Ok(HttpResponse::Ok().json(attachment))
}

#[derive(Deserialize)]
pub struct AttachmentParams {
    #[serde(rename = "nodeId")]
    node_id: Uuid,

    #[serde(rename = "objectId")]
    object_id: Uuid,

    filename: String,
}

#[get("/presigned_url")]
pub async fn get_presigned_url(
    params: web::Query<AttachmentParams>,
    app: web::Data<App>,
    current_user: CurrentUser,
) -> Response {
    auth_node_update_by_id(&params.node_id, &app.db_session, &current_user).await?;

    let url = get_s3_presigned_url(&app, &params.filename).await?;

    Ok(HttpResponse::Ok().json(json!({
        "url": url,
        "filename": params.filename,
        "nodeId": params.node_id,
        "objectId": params.object_id,
    })))
}

#[post("")]
pub async fn create_attachment(
    mut attachment: web::Json<Attachment>,
    app: web::Data<App>,
    current_user: CurrentUser,
) -> Response {
    let url = Attachment::build_s3_url(app.s3_bucket.clone(), attachment.key.clone());

    attachment.url = Some(url);
    attachment.user_id = Some(current_user.id);

    attachment.insert_cb(&app.db_session).await?;

    Ok(HttpResponse::Ok().json(attachment))
}
