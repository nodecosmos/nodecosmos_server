use crate::api::data::RequestData;
use crate::api::types::Response;
use crate::constants::PAGE_SIZE;
use crate::models::notification::Notification;
use actix_web::{get, post, web, HttpResponse};
use base64::engine::general_purpose::URL_SAFE;
use base64::Engine;
use charybdis::errors::CharybdisError;
use charybdis::operations::Find;
use charybdis::scylla::PagingState;
use scylla::statement::PagingStateResponse;
use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize)]
pub struct PagedQuery {
    pub paging_state: Option<String>,
    pub new: Option<bool>,
}

#[get("/")]
pub async fn get_notifications(data: RequestData, q: web::Query<PagedQuery>) -> Response {
    let paging_state = if let Some(q) = q.paging_state.as_ref() {
        Some(PagingState::new_from_raw_bytes(URL_SAFE.decode(&q)?))
    } else {
        None
    };

    let query = Notification::find_by_partition_key_value_paged((data.current_user.id,));

    let (notifications, paging_state_response) = if q.new.unwrap_or(false) {
        let notifications = Notification::find_by_user_id_and_seen(data.current_user.id, false)
            .execute(data.db_session())
            .await?
            .try_collect()
            .await?;

        (notifications, None::<PagingStateResponse>)
    } else {
        let (notification_res, paging_state_response) = query
            .page_size(PAGE_SIZE)
            .paging_state(paging_state.unwrap_or_default())
            .execute(data.db_session())
            .await?;

        let notifications: Result<Vec<Notification>, CharybdisError> = notification_res.collect();

        (notifications?, Some(paging_state_response))
    };

    let paging_state_response = paging_state_response.map(|psr| match psr {
        PagingStateResponse::HasMorePages { state } => state.as_bytes_slice().map(|b| URL_SAFE.encode(b)),
        PagingStateResponse::NoMorePages => None,
    });

    Ok(HttpResponse::Ok().json(json!({
        "notifications": notifications,
        "pagingState": paging_state_response,
    })))
}

#[post("/mark_all_as_read")]
pub async fn mark_all_as_read(data: RequestData) -> Response {
    Notification::mark_all_as_read(data.db_session(), data.current_user.id).await?;

    Ok(HttpResponse::Ok().finish())
}
