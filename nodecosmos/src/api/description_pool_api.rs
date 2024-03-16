use crate::api::data::RequestData;
use crate::api::types::Response;
use crate::models::node::Node;
use crate::models::traits::node::FindBranched;
use crate::models::traits::Authorization;
use crate::resources::description_ws_pool::DescriptionWsConnection;
use actix_web::{get, web, HttpRequest};
use actix_web_actors::ws::WsResponseBuilder;
use charybdis::types::Uuid;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct PathParams {
    // check if we can route Load Balancer connections based on room_id params
    room_id: Uuid,
    branch_id: Uuid,
    node_id: Uuid,
}

/// Websocket connection to sync description updates
/// between attached clients.
/// It can be used for all models that have description_base64 field.
#[get("/description/{node_id}/{branch_id}/{room_id}")]
pub async fn description_ws(
    req: HttpRequest,
    stream: web::Payload,
    params: web::Path<PathParams>,
    data: RequestData,
) -> Response {
    let mut node = Node {
        id: params.node_id,
        branch_id: params.branch_id,
        ..Default::default()
    };
    node.find_branched(data.db_session()).await?;

    node.auth_update(&data).await?;

    let ws_desc_conn = DescriptionWsConnection {
        room_id: params.room_id,
        pool: data.description_ws_pool(),
    };
    let ws_builder = WsResponseBuilder::new(ws_desc_conn.clone(), &req, stream);
    let (addr, resp) = ws_builder.start_with_addr()?;
    let ws_desc_conn_pool = ws_desc_conn.pool;
    ws_desc_conn_pool
        .connections
        .entry(params.room_id)
        .or_default()
        .push(addr);

    Ok(resp)
}
