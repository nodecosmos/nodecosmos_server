use crate::actions::client_session::CurrentUser;
use crate::errors::NodecosmosError;
use crate::models::input_output::InputOutput;

use actix_web::{post, web, HttpResponse};
use charybdis::InsertWithCallbacks;
use scylla::CachingSession;
use serde_json::json;

#[post("")]
pub async fn create_io(
    db_session: web::Data<CachingSession>,
    _current_user: CurrentUser,
    input_output: web::Json<InputOutput>,
) -> Result<HttpResponse, NodecosmosError> {
    let mut input_output = input_output.into_inner();

    input_output.insert_cb(&db_session).await?;

    Ok(HttpResponse::Ok().json(json!({
        "success": true,
        "inputOutput": input_output,
    })))
}
