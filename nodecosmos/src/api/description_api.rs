use actix_web::{get, post, web, HttpRequest, HttpResponse};
use actix_web_actors::ws::WsResponseBuilder;
use charybdis::operations::InsertWithCallbacks;
use charybdis::types::Uuid;
use serde::Deserialize;

use crate::api::current_user::OptCurrentUser;
use crate::api::data::RequestData;
use crate::api::types::Response;
use crate::app::App;
use crate::errors::NodecosmosError;
use crate::models::description::{BaseDescription, Description};
use crate::models::node::AuthNode;
use crate::models::traits::Branchable;
use crate::resources::description_ws_pool::DescriptionWsConnection;

#[get("/{nodeId}/{objectId}/{objectType}/{branchId}")]
pub async fn get_description(
    app: web::Data<App>,
    opt_cu: OptCurrentUser,
    mut description: web::Path<BaseDescription>,
) -> Response {
    AuthNode::auth_view(&app, opt_cu, description.node_id, description.branch_id).await?;

    let description = description.find_branched(&app.db_session).await?;

    Ok(HttpResponse::Ok().json(description))
}

#[get("/{nodeId}/{objectId}/{objectType}/{branchId}/base64")]
pub async fn get_base64_description(data: RequestData, mut description: web::Path<Description>) -> Response {
    AuthNode::auth_update(&data, description.node_id, description.branch_id).await?;

    description.find_branched(data.db_session()).await?;

    // we always return merged description as we want to keep branched description in sync with original
    if description.is_branched() {
        let original = Description::find_by_object_id_and_branch_id(description.object_id, description.original_id())
            .execute(data.db_session())
            .await;

        if let Ok(mut original) = original {
            match description.base64 {
                Some(_) => {
                    if description.base64 != original.base64 {
                        original.merge(&description).await?;
                        description.html = original.html;
                        description.markdown = original.markdown;
                        description.base64 = original.base64;
                    }
                }

                None => {
                    description.base64 = original.base64;
                }
            }
        }
    }

    Ok(HttpResponse::Ok().json(description.into_inner()))
}

#[derive(Deserialize)]
pub struct OriginalPathParams {
    #[serde(rename = "nodeId")]
    pub node_id: Uuid,

    #[serde(rename = "objectId")]
    pub object_id: Uuid,
}

#[get("/{nodeId}/{objectId}")]
pub async fn get_original_description(
    app: web::Data<App>,
    opt_cu: OptCurrentUser,
    params: web::Path<OriginalPathParams>,
) -> Response {
    AuthNode::auth_view(&app, opt_cu, params.node_id, params.node_id).await?;

    let description = Description::find_by_object_id_and_branch_id(params.object_id, params.object_id)
        .execute(&app.db_session)
        .await?;

    Ok(HttpResponse::Ok().json(description))
}

#[post("")]
pub async fn save_description(data: RequestData, mut description: web::Json<Description>) -> Response {
    AuthNode::auth_update(&data, description.node_id, description.branch_id).await?;

    description.insert_cb(&data).execute(data.db_session()).await?;

    Ok(HttpResponse::Ok().json(description))
}

#[derive(Deserialize)]
pub struct WsPathParams {
    // TODO: check if we can route Load Balancer connections based on room_id params
    room_id: Uuid,
    branch_id: Uuid,
    node_id: Uuid,
}

/// Websocket connection to sync description updates
/// between attached clients.
/// It can be used for all models that have description_base64 field.
#[get("/descriptions/{node_id}/{branch_id}/{room_id}")]
pub async fn description_ws(
    req: HttpRequest,
    stream: web::Payload,
    params: web::Path<WsPathParams>,
    data: RequestData,
) -> Response {
    AuthNode::auth_update(&data, params.node_id, params.branch_id).await?;

    let ws_desc_conn = DescriptionWsConnection {
        room_id: params.room_id,
        pool: data.description_ws_pool(),
    };
    let ws_builder = WsResponseBuilder::new(ws_desc_conn.clone(), &req, stream);
    let (addr, resp) = ws_builder
        .start_with_addr()
        .map_err(|e| NodecosmosError::InternalServerError(format!("Failed to start websocket connection: {}", e)))?;

    let ws_desc_conn_pool = ws_desc_conn.pool;
    ws_desc_conn_pool
        .connections
        .entry(params.room_id)
        .or_default()
        .push(addr);

    Ok(resp)
}
