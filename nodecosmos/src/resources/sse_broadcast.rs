use charybdis::model::BaseModel;
use charybdis::serializers::ToJson;
use charybdis::types::Uuid;
use dashmap::DashMap;
use log::error;
use redis::{FromRedisValue, RedisWrite, ToRedisArgs, Value};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fmt::Display;
use tokio::sync::broadcast;

use crate::api::data::RequestData;
use crate::api::types::ActionTypes;
use crate::errors::NodecosmosError;

#[derive(Debug, Serialize, Deserialize)]
pub struct SseMessage {
    pub root_id: Uuid,
    pub data: String,
}

impl Display for SseMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "root_id:{}\ndata: {}", &self.root_id, &self.data)
    }
}

impl ToRedisArgs for SseMessage {
    fn to_redis_args(&self) -> Vec<Vec<u8>> {
        vec![json!(self).to_string().as_bytes().to_vec()]
    }

    fn write_redis_args<W>(&self, out: &mut W)
    where
        W: ?Sized + RedisWrite,
    {
        out.write_arg(&json!(self).to_string().as_bytes())
    }
}

impl FromRedisValue for SseMessage {
    fn from_redis_value(v: &Value) -> redis::RedisResult<Self> {
        match v {
            Value::BulkString(data) => {
                let data = String::from_utf8(data.to_vec()).expect("Invalid UTF-8 sequence");
                let msg: SseMessage = serde_json::from_str(&data).expect("Invalid JSON");

                Ok(msg)
            }
            _ => Err(redis::RedisError::from((
                redis::ErrorKind::TypeError,
                "Invalid data type",
            ))),
        }
    }
}

#[derive(Debug)]
pub struct SseBroadcast {
    pub root_channels: DashMap<Uuid, broadcast::Sender<actix_web::web::Bytes>>,
}

impl SseBroadcast {
    pub fn new() -> Self {
        SseBroadcast {
            root_channels: DashMap::new(),
        }
    }

    pub async fn send_message(&self, root_id: Uuid, model_msg: actix_web::web::Bytes) -> Result<(), NodecosmosError> {
        if let Some(sender) = self.root_channels.get(&root_id) {
            sender.send(model_msg).map_err(|e| {
                error!("Error sending message to room {}: {}", root_id, e);

                NodecosmosError::BroadcastError(format!("Error sending message to room {}: {}", root_id, e))
            })?;
        }

        Ok(())
    }

    pub fn ping_channels(&self) {
        for channel in self.root_channels.iter() {
            let root_id = channel.key();
            if let Some(sender) = self.root_channels.get(root_id) {
                let _ = sender.send(actix_web::web::Bytes::from("event: PING\ndata: PONG\n\n"));
            }
        }
    }

    pub fn build_receiver(&self, root_id: Uuid) -> broadcast::Receiver<actix_web::web::Bytes> {
        let receiver;

        if let Some(sender) = self.root_channels.get(&root_id) {
            receiver = sender.subscribe();
        } else {
            let (tx, rx) = broadcast::channel(20);
            self.root_channels.insert(root_id, tx);
            receiver = rx;
        }

        receiver
    }

    pub fn cleanup_rooms(&self) {
        self.root_channels.retain(|_, v| {
            log::info!("Cleaning up room: {}", v.receiver_count());

            v.receiver_count() > 0
        });
    }

    // this will broadcast message to all app instances through redis
    async fn broadcast_message(&self, data: &RequestData, msg: SseMessage) -> Result<(), NodecosmosError> {
        let mut connection = data.redis_connection().await?;

        redis::cmd("PUBLISH")
            .arg("BROADCAST_MESSAGE")
            .arg(&msg)
            .query_async::<()>(&mut *connection)
            .await
            .map_err(|e| {
                NodecosmosError::LockerError(format!(
                    "[query_async] Failed to publish message {}! Error: {:?}",
                    msg, e
                ))
            })?;

        Ok(())
    }
}

pub struct ModelEvent<'a, M: BaseModel> {
    pub root_id: Uuid,
    pub action_type: ActionTypes,
    pub model: &'a M,
}

impl<'a, M: BaseModel + Serialize> ModelEvent<'a, M> {
    pub fn new(root_id: Uuid, action_type: ActionTypes, model: &'a M) -> Self {
        ModelEvent {
            root_id,
            action_type,
            model,
        }
    }

    pub async fn send(&self, data: &RequestData) -> Result<(), NodecosmosError> {
        let sse_broadcast = data.sse_broadcast();
        let msg = self.to_sse()?;

        sse_broadcast.broadcast_message(data, msg).await
    }

    fn to_sse(&self) -> Result<SseMessage, NodecosmosError> {
        Ok(SseMessage {
            root_id: self.root_id,
            data: format!("event: {}\ndata: {}\n\n", self.action_type, self.model.to_json()?),
        })
    }
}
