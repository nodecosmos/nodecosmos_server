use crate::api::data::RequestData;
use crate::api::types::Response;
use crate::constants::PAGE_SIZE;
use crate::models::notification::Notification;
use actix_web::{get, post, web, HttpResponse};
use base64::engine::general_purpose::URL_SAFE;
use base64::Engine;
use charybdis::errors::CharybdisError;
use charybdis::operations::Find;
use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize)]
pub struct PagedQuery {
    pub paging_state: Option<String>,
}

#[get("/")]
pub async fn get_notifications(data: RequestData, q: web::Query<PagedQuery>) -> Response {
    let query = Notification::find_by_partition_key_value_paged((data.current_user.id,));

    let paging_state = if let Some(q) = q.paging_state.as_ref() {
        Some(scylla::Bytes::from(URL_SAFE.decode(&q)?))
    } else {
        None
    };

    let (notification_res, paging_state) = query
        .page_size(PAGE_SIZE)
        .paging_state(paging_state)
        .execute(data.db_session())
        .await?;

    let notifications: Result<Vec<Notification>, CharybdisError> = notification_res.collect();
    let paging_state = paging_state.map(|p| URL_SAFE.encode(&p));

    Ok(HttpResponse::Ok().json(json!({
        "notifications": notifications?,
        "paging_state": paging_state,
    })))
}

#[post("/mark_all_as_read")]
pub async fn mark_all_as_read(data: RequestData) -> Response {
    Notification::mark_all_as_read(data.db_session(), data.current_user.id).await?;

    Ok(HttpResponse::Ok().finish())
}
