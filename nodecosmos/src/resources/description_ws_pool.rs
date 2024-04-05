use actix::prelude::*;
use actix_web_actors::ws;
use charybdis::types::Uuid;
use dashmap::DashMap;
use log::error;
use std::sync::Arc;

type RoomId = Uuid;

#[derive(Default)]
pub struct DescriptionWsPool {
    pub connections: DashMap<RoomId, Vec<Addr<DescriptionWsConnection>>>,
}

#[derive(Clone)]
pub struct DescriptionWsConnection {
    pub room_id: Uuid,
    pub pool: Arc<DescriptionWsPool>,
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
