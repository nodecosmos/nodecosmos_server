use crate::api::data::RequestData;
use crate::api::types::ActionTypes;
use crate::errors::NodecosmosError;
use actix_web::web;
use actix_web::web::Bytes;
use charybdis::model::BaseModel;
use charybdis::serializers::ToJson;
use charybdis::types::Uuid;
use dashmap::DashMap;
use log::error;
use serde::Serialize;
use tokio::sync::broadcast;

pub struct SseBroadcast {
    node_senders: DashMap<Uuid, broadcast::Sender<Bytes>>,
}

impl SseBroadcast {
    pub fn new() -> Self {
        SseBroadcast {
            node_senders: DashMap::new(),
        }
    }

    pub async fn send_message(&self, node_id: &Uuid, model_msg: Bytes) -> Result<(), NodecosmosError> {
        if let Some(sender) = self.node_senders.get(&node_id) {
            sender.send(model_msg).map_err(|e| {
                error!("Error sending message to room {}: {}", node_id, e);

                NodecosmosError::BroadcastError(e)
            })?;
        }

        Ok(())
    }

    pub fn get_or_create_room(&self, node_id: Uuid) -> broadcast::Sender<Bytes> {
        let sender = self
            .node_senders
            .entry(node_id)
            .or_insert_with(|| broadcast::channel(20).0.clone());

        sender.clone()
    }
}

pub struct ModelEvent<'a, M: BaseModel + Serialize> {
    pub node_id: &'a Uuid,
    pub action_type: ActionTypes,
    pub model: &'a M,
}

impl<'a, M: BaseModel + Serialize> ModelEvent<'a, M> {
    pub fn new(node_id: &'a Uuid, action_type: ActionTypes, model: &'a M) -> Self {
        ModelEvent {
            node_id,
            action_type,
            model,
        }
    }

    fn to_sse(&self) -> Result<Bytes, NodecosmosError> {
        let msg = web::Bytes::from(format!(
            "event: {}\ndata: {}\n\n",
            self.action_type.to_string(),
            self.model.to_json()?
        ));

        Ok(msg)
    }

    pub async fn send(&self, request_data: &RequestData) -> Result<(), NodecosmosError> {
        let sse_broadcast = request_data.sse_broadcast();
        let msg = self.to_sse()?;

        sse_broadcast.send_message(self.node_id, msg).await
    }
}
