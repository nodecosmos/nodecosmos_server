use crate::api::data::RequestData;
use crate::api::types::Response;
use crate::models::node::Node;
use crate::models::traits::Authorization;
use actix::prelude::*;
use actix_web::{get, web, HttpRequest};
use actix_web_actors::ws;
use actix_web_actors::ws::WsResponseBuilder;
use charybdis::operations::Find;
use charybdis::types::Uuid;
use dashmap::DashMap;
use serde::Deserialize;
use std::sync::Arc;

type RoomId = Uuid;

#[derive(Default)]
pub struct DescriptionWsConnectionPool {
    pub connections: DashMap<RoomId, Vec<Addr<DescriptionWsConnection>>>,
}

#[derive(Clone)]
pub struct DescriptionWsConnection {
    pub room_id: Uuid,
    pub pool: Arc<DescriptionWsConnectionPool>,
}

impl Actor for DescriptionWsConnection {
    type Context = ws::WebsocketContext<Self>;
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for DescriptionWsConnection {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(ws::Message::Ping(msg)) => ctx.pong(&msg),
            Ok(ws::Message::Binary(bin)) => {
                if let Some(connections) = self.pool.connections.get(&self.room_id) {
                    for conn in connections.iter() {
                        let address = ctx.address();
                        // this will call `handle` method of `DescriptionUpdateMessage` actor
                        let message = DescriptionUpdateMessage {
                            message: ws::Message::Binary(bin.clone()),
                            origin_address: address,
                        };
                        conn.do_send(message);
                    }
                } else {
                    ctx.close(None);
                    ctx.stop();
                    println!("No connections for node {}", self.room_id);
                }
            }
            Ok(ws::Message::Close(reason)) => {
                ctx.close(reason);
                ctx.stop();
                self.finished(ctx);
            }
            _ => (),
        }
    }

    fn finished(&mut self, ctx: &mut Self::Context) {
        if let Some(mut connections) = self.pool.connections.get_mut(&self.room_id) {
            connections.retain(|addr| *addr != ctx.address());
        }

        ctx.stop();
    }
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct DescriptionUpdateMessage {
    pub message: ws::Message,
    pub origin_address: Addr<DescriptionWsConnection>,
}

impl Handler<DescriptionUpdateMessage> for DescriptionWsConnection {
    type Result = ();

    fn handle(&mut self, msg: DescriptionUpdateMessage, ctx: &mut Self::Context) {
        if msg.origin_address != ctx.address() {
            match msg.message {
                ws::Message::Binary(bin) => ctx.binary(bin),
                ws::Message::Text(text) => ctx.text(text),
                _ => (),
            }
        }
    }
}

#[derive(Deserialize)]
pub struct PathParams {
    // check if we can route Load Balancer connections based on room_id params
    room_id: Uuid,
    branch_id: Uuid,
    node_id: Uuid,
}

/// Websocket connection to sync description updates
/// between attached clients.
///
/// It can be used for all models that have description_base64 field.
#[get("/description/{node_id}/{branch_id}/{room_id}")]
pub async fn description_ws(
    req: HttpRequest,
    stream: web::Payload,
    params: web::Path<PathParams>,
    node_ws_desc_conn_pool: web::Data<DescriptionWsConnectionPool>,
    data: RequestData,
) -> Response {
    let mut node = Node {
        id: params.node_id,
        branch_id: params.branch_id,
        ..Default::default()
    };
    node.transform_to_branched(data.db_session()).await?;

    node.auth_update(&data).await?;

    let ws_desc_conn = DescriptionWsConnection {
        room_id: params.room_id,
        pool: node_ws_desc_conn_pool.into_inner(),
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
