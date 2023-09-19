use crate::actions::client_session::CurrentUser;
use crate::actions::NodePrimaryKeyParams;
use crate::authorize::auth_node_update;
use crate::errors::NodecosmosError;
use crate::models::node::Node;
use actix::prelude::*;
use actix_web::{get, web, HttpRequest, HttpResponse};
use actix_web_actors::ws;
use actix_web_actors::ws::WsResponseBuilder;
use charybdis::{Find, Uuid};
use dashmap::DashMap;
use scylla::CachingSession;
use std::sync::Arc;

type NodeId = Uuid;

#[derive(Default)]
pub struct NodeDescriptionWsConnectionPool {
    pub connections: DashMap<NodeId, Vec<Addr<NodeDescriptionWsConnection>>>,
}

#[derive(Message, Clone)]
#[rtype(result = "()")]
pub struct NodeDescriptionWsConnection {
    pub id: Uuid,
    pub pool: Arc<NodeDescriptionWsConnectionPool>,
}

impl Actor for NodeDescriptionWsConnection {
    type Context = ws::WebsocketContext<Self>;
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for NodeDescriptionWsConnection {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(ws::Message::Ping(msg)) => ctx.pong(&msg),
            Ok(ws::Message::Binary(bin)) => {
                if let Some(connections) = self.pool.connections.get(&self.id) {
                    for conn in connections.iter() {
                        let address = ctx.address();
                        // this will call `handle` method of `NodeDescriptionUpdateMessage` actor
                        let message = NodeDescriptionUpdateMessage {
                            message: ws::Message::Binary(bin.clone()),
                            origin_address: address,
                        };
                        conn.do_send(message);
                    }
                } else {
                    ctx.close(None);
                    ctx.stop();
                    println!("No connections for node {}", self.id);
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
        if let Some(mut connections) = self.pool.connections.get_mut(&self.id) {
            connections.retain(|addr| *addr != ctx.address());
        }

        ctx.stop();
    }
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct NodeDescriptionUpdateMessage {
    pub message: ws::Message,
    pub origin_address: Addr<NodeDescriptionWsConnection>,
}

impl Handler<NodeDescriptionUpdateMessage> for NodeDescriptionWsConnection {
    type Result = ();

    fn handle(&mut self, msg: NodeDescriptionUpdateMessage, ctx: &mut Self::Context) {
        if msg.origin_address != ctx.address() {
            match msg.message {
                ws::Message::Binary(bin) => ctx.binary(bin),
                ws::Message::Text(text) => ctx.text(text),
                _ => (),
            }
        }
    }
}

#[get("/ws/node_description/{root_id}/{id}")]
pub async fn node_description_ws(
    req: HttpRequest,
    stream: web::Payload,
    params: web::Path<NodePrimaryKeyParams>,
    current_user: CurrentUser,
    db_session: web::Data<CachingSession>,
    node_ws_desc_conn_pool: web::Data<NodeDescriptionWsConnectionPool>,
) -> Result<HttpResponse, NodecosmosError> {
    let node = Node {
        root_id: params.root_id,
        id: params.id,
        ..Default::default()
    }
    .find_by_primary_key(&db_session)
    .await?;

    auth_node_update(&node, &current_user).await?;

    let node_ws_desc_conn = NodeDescriptionWsConnection {
        id: node.id,
        pool: node_ws_desc_conn_pool.into_inner(),
    };

    let ws_builder = WsResponseBuilder::new(node_ws_desc_conn.clone(), &req, stream);
    let (addr, resp) = ws_builder.start_with_addr()?;
    let node_ws_desc_conn_pool = node_ws_desc_conn.pool;
    node_ws_desc_conn_pool
        .connections
        .entry(node.id)
        .or_default()
        .push(addr);

    Ok(resp)
}
