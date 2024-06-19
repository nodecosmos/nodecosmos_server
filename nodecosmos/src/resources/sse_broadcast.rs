use actix_web::web;
use actix_web::web::Bytes;
use charybdis::model::BaseModel;
use charybdis::serializers::ToJson;
use charybdis::types::Uuid;
use dashmap::DashMap;
use log::error;
use serde::Serialize;
use tokio::sync::broadcast;

use crate::api::data::RequestData;
use crate::api::types::ActionTypes;
use crate::errors::NodecosmosError;

#[derive(Debug)]
pub struct SseBroadcast {
    pub root_channels: DashMap<Uuid, broadcast::Sender<Bytes>>,
}

impl SseBroadcast {
    pub fn new() -> Self {
        SseBroadcast {
            root_channels: DashMap::new(),
        }
    }

    pub async fn send_message(&self, root_id: Uuid, model_msg: Bytes) -> Result<(), NodecosmosError> {
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
            if let Some(sender) = self.root_channels.get(&root_id) {
                let _ = sender.send(web::Bytes::from("event: PING\ndata: PONG\n\n"));
            }
        }
    }

    pub fn build_receiver(&self, root_id: Uuid) -> broadcast::Receiver<Bytes> {
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

            return v.receiver_count() > 0;
        });
    }
}

pub struct ModelEvent<'a, M: BaseModel + Serialize> {
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

        println!("Sending message: {:?}", msg);

        sse_broadcast.send_message(self.root_id, msg).await
    }

    fn to_sse(&self) -> Result<Bytes, NodecosmosError> {
        let msg = web::Bytes::from(format!(
            "event: {}\ndata: {}\n\n",
            self.action_type.to_string(),
            self.model.to_json()?
        ));

        Ok(msg)
    }
}
