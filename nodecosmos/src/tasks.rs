use crate::api::data::RequestData;
use crate::app::App;
use crate::resources::sse_broadcast::{SseBroadcast, SseMessage};
use futures::StreamExt;
use log::info;
use rand::prelude::IndexedRandom;
use redis::ConnectionAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::time;

pub async fn recovery_task(data: RequestData) {
    let interval_sec = crate::models::recovery::RECOVERY_INTERVAL_MIN * 60;
    let mut recovery_interval = time::interval(Duration::from_secs(interval_sec as u64));

    tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = recovery_interval.tick() => {
                    let _ = crate::models::recovery::Recovery::run_recovery_task(&data.clone())
                        .await
                        .map_err(|e| {
                            log::error!("Recovery task failed: {:?}", e);
                        });
                    info!("Recovery task ran");
                }
                _ = tokio::signal::ctrl_c() => {
                    info!("Recovery task is shutting down due to Ctrl-C.");
                    break;
                }
            }
        }
    });
}

pub async fn cleanup_rooms_task(sse_broadcast: Arc<SseBroadcast>) {
    let mut cleanup_interval = time::interval(Duration::from_secs(600));
    let sse_broadcast_clone = sse_broadcast.clone();

    tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = cleanup_interval.tick() => {
                    sse_broadcast_clone.ping_channels();
                    sse_broadcast_clone.cleanup_rooms();
                    info!("Cleanup rooms task ran");
                }
                _ = tokio::signal::ctrl_c() => {
                    info!("Cleanup room task is shutting down due to Ctrl-C.");
                    break;
                }
            }
        }
    });
}

pub async fn listen_redis_events(app: &App) {
    // TODO: get redis client from the same zone with app.find_local_zone_redis_client
    let client = app.redis_clients.first().expect("Redis should have one client");

    let mut pubsub = client.get_async_pubsub().await.expect("Failed to get redis connection");
    let sse_broadcast = app.sse_broadcast.clone();

    tokio::spawn(async move {
        pubsub
            .subscribe("BROADCAST_MESSAGE")
            .await
            .expect("Failed to subscribe to channel");

        let mut on_message = pubsub.on_message();

        loop {
            tokio::select! {
                msg = on_message.next() => {
                    if let Some(msg) = msg {
                        let payload = msg.get_payload::<SseMessage>();
                        match payload {
                            Ok(payload) => {
                                let _ = sse_broadcast
                                    .send_message(payload.root_id, actix_web::web::Bytes::from(payload.data))
                                    .await
                                    .map_err(|e| {
                                        log::error!("Failed to send message: {:?}", e);
                                    });
                            }
                            Err(e) => {
                                log::error!("Failed to get payload: {}", e);
                            }
                        }
                    }
                }
                _ = tokio::signal::ctrl_c() => {
                    info!("SSE task is shutting down due to Ctrl-C.");
                    break;
                }
            }
        }
    });
}
