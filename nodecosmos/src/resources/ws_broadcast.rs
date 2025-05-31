use std::sync::Arc;

use actix::prelude::*;
use actix_web_actors::ws;
use dashmap::DashMap;
use log::error;

type RoomId = String; // BranchId + RoomId

#[derive(Default)]
pub struct WsBroadcast {
    pub connections: DashMap<RoomId, Vec<Addr<WsConnection>>>,
}

#[derive(Clone)]
pub struct WsConnection {
    pub room_id: RoomId,
    pub broadcast: Arc<WsBroadcast>,
}

impl Actor for WsConnection {
    type Context = ws::WebsocketContext<Self>;
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for WsConnection {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(ws::Message::Ping(msg)) => ctx.pong(&msg),
            Ok(ws::Message::Binary(bin)) => {
                if let Some(connections) = self.broadcast.connections.get(&self.room_id) {
                    for conn in connections.iter() {
                        let address = ctx.address();
                        // this will call `handle` method of `WsMessage` actor
                        let message = WsMessage {
                            message: ws::Message::Binary(bin.clone()),
                            origin_address: address,
                        };
                        conn.do_send(message);
                    }
                } else {
                    ctx.close(None);
                    ctx.stop();
                    error!("No connections for node {}", self.room_id);
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
        if let Some(mut connections) = self.broadcast.connections.get_mut(&self.room_id) {
            connections.retain(|addr| *addr != ctx.address());
        }

        ctx.stop();
    }
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct WsMessage {
    pub message: ws::Message,
    pub origin_address: Addr<WsConnection>,
}

impl Handler<WsMessage> for WsConnection {
    type Result = ();

    fn handle(&mut self, msg: WsMessage, ctx: &mut Self::Context) {
        if msg.origin_address != ctx.address() {
            match msg.message {
                ws::Message::Binary(bin) => ctx.binary(bin),
                ws::Message::Text(text) => ctx.text(text),
                _ => (),
            }
        }
    }
}
