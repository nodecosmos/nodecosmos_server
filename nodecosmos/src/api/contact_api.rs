use crate::api::current_user::OptCurrentUser;
use crate::api::types::Response;
use crate::app::App;
use crate::models::contact::Contact;
use actix_web::{post, web, HttpResponse};
use charybdis::operations::InsertWithCallbacks;

#[post("/contact_us")]
pub async fn create_contact_us(
    mut contact: web::Json<Contact>,
    opt_cu: OptCurrentUser,
    app: web::Data<App>,
) -> Response {
    contact.user_id = opt_cu.0.map(|cu| cu.id);

    contact.insert_cb(&app).execute(&app.db_session).await?;

    Ok(HttpResponse::Ok().finish())
}
