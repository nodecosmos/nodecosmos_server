use crate::api::data::RequestData;
use crate::api::types::Response;
use crate::models::notification::Notification;
use actix_web::{get, post, web, HttpResponse};
use base64::engine::general_purpose::URL_SAFE;
use base64::Engine;
use charybdis::errors::CharybdisError;
use charybdis::operations::Find;
use serde::Deserialize;
use serde_json::json;

#[get("/")]
pub async fn get_notifications(data: RequestData) -> Response {
    let notifications = Notification::find_by_user_id_and_seen(data.current_user.id, false)
        .execute(data.db_session())
        .await?
        .try_collect()
        .await?;

    Ok(HttpResponse::Ok().json(notifications))
}

#[derive(Deserialize)]
pub struct PagedQuery {
    pub paging_state: Option<String>,
}

#[get("/all_paged")]
pub async fn get_all_paged(data: RequestData, q: web::Query<PagedQuery>) -> Response {
    let query = Notification::find_by_partition_key_value_paged((data.current_user.id,)).page_size(50);
    if let Some(q) = q.paging_state.as_ref() {
        let paging_state = URL_SAFE.decode(&q)?;
        query.paging_state(Some(scylla::Bytes::from(paging_state)));
    }

    let (notification_res, paging_state) = Notification::find_by_partition_key_value_paged((data.current_user.id,))
        .page_size(10)
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
