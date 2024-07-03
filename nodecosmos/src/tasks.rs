use crate::api::data::RequestData;
use crate::resources::sse_broadcast::SseBroadcast;
use log::info;
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
                    info!("Cleanup task ran");
                }
                _ = tokio::signal::ctrl_c() => {
                    info!("Recovery task is shutting down due to Ctrl-C.");
                    break;
                }
            }
        }
    });
}
