#![allow(incomplete_features)]
#![feature(async_fn_in_trait)]
#![feature(const_option)]

mod actions;
mod app;
mod authorize;
mod callback_extension;
mod errors;
mod models;
mod services;

use crate::app::*;
use crate::models::node_descendant::NodeDescendant;
use crate::services::elastic;
use crate::services::resource_locker::ResourceLocker;
use actions::*;
use actix_web::middleware::Logger;
use actix_web::{web, App, HttpServer};
use app::App as NodecosmosApp;
pub use callback_extension::CbExtension;
use deadpool_redis::Pool;
use std::sync::Arc;

#[tokio::main]
async fn main() {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    let nodecosmos = NodecosmosApp::new();
    let nodecosmos_web_data = web::Data::new(nodecosmos.clone());

    let port = get_port(&nodecosmos);

    // web data
    let db_session = get_db_session(&nodecosmos).await;
    let db_session_web_data = web::Data::new(db_session);

    let elastic_client = get_elastic_client(&nodecosmos).await;
    let elastic_client_web_data = web::Data::new(elastic_client.clone());

    let s3_client = get_aws_s3_client().await;
    let s3_client_web_data = web::Data::new(s3_client.clone());

    let desc_ws_conn_pool = web::Data::new(DescriptionWsConnectionPool::default());

    let redis_pool: Pool = get_redis_pool(&nodecosmos).await;
    let redis_pool_web_data = web::Data::new(redis_pool.clone());

    let redis_pool_arc = Arc::clone(&redis_pool_web_data);
    let resource_locker = ResourceLocker::new(redis_pool_arc);
    let resource_locker_web_data = web::Data::new(resource_locker.clone());

    let cb_extension = CbExtension::new(elastic_client.clone(), resource_locker.clone());
    let cb_extension_web_data = web::Data::new(cb_extension.clone());

    let vec_size = std::mem::size_of::<Vec<NodeDescendant>>();
    let node_descendant_size = std::mem::size_of::<NodeDescendant>();

    println!("Vec<NodeDescendant> size: {}", vec_size);
    println!("NodeDescendant size: {}", node_descendant_size);

    nodecosmos
        .init(
            &db_session_web_data,
            &resource_locker_web_data,
            &elastic_client,
        )
        .await;

    HttpServer::new(move || {
        App::new()
            .wrap(Logger::new("%a %r %s %b %{Referer}i %{User-Agent}i %T"))
            .wrap(get_cors(&nodecosmos))
            .wrap(get_session_middleware(&nodecosmos))
            .app_data(db_session_web_data.clone())
            .app_data(elastic_client_web_data.clone())
            .app_data(cb_extension_web_data.clone())
            .app_data(redis_pool_web_data.clone())
            .app_data(resource_locker_web_data.clone())
            .app_data(s3_client_web_data.clone())
            .app_data(nodecosmos_web_data.clone())
            .app_data(desc_ws_conn_pool.clone())
            .service(web::scope("/ws").service(description_ws))
            .service(
                web::scope("/users")
                    .service(get_user)
                    .service(create_user)
                    .service(update_user)
                    .service(delete_user),
            )
            .service(
                web::scope("/sessions")
                    .service(login)
                    .service(sync)
                    .service(logout),
            )
            .service(
                web::scope("/nodes")
                    .service(get_nodes)
                    .service(get_node)
                    .service(create_node)
                    .service(update_node_title)
                    .service(update_node_description)
                    .service(delete_node)
                    .service(get_node_description)
                    .service(get_node_description_base64)
                    .service(reorder_nodes)
                    .service(upload_cover_image)
                    .service(delete_cover_image),
            )
            .service(
                web::scope("/likes")
                    .service(liked_object_ids)
                    .service(get_likes_count)
                    .service(create_like)
                    .service(delete_like),
            )
            .service(
                web::scope("/workflows")
                    .service(get_workflow)
                    .service(create_workflow)
                    .service(update_initial_inputs)
                    .service(update_workflow_title)
                    .service(delete_workflow),
            )
            .service(
                web::scope("/flows")
                    .service(create_flow)
                    .service(get_flow_description)
                    .service(update_flow_title)
                    .service(update_flow_description)
                    .service(delete_flow),
            )
            .service(
                web::scope("/flow_steps")
                    .service(create_flow_step)
                    .service(update_flow_step_nodes)
                    .service(update_flow_step_inputs)
                    .service(update_flow_step_outputs)
                    .service(update_flow_step_description)
                    .service(delete_flow_step),
            )
            .service(
                web::scope("input_outputs")
                    .service(create_io)
                    .service(get_io_description)
                    .service(update_io_title)
                    .service(update_io_description)
                    .service(delete_io),
            )
            .service(
                web::scope("contribution_requests")
                    .service(get_contribution_requests)
                    .service(get_contribution_request)
                    .service(create_contribution_request)
                    .service(update_contribution_request_title)
                    .service(update_contribution_request_description)
                    .service(delete_contribution_request),
            )
            .service(
                web::scope("commits")
                    .service(create_node_commit)
                    .service(update_node_commit_title)
                    .service(update_node_commit_description)
                    .service(delete_node_commit)
                    .service(create_workflow_commit),
            )
            .service(
                web::scope("attachments")
                    .service(upload_image)
                    .service(get_presigned_url)
                    .service(create_attachment),
            )
    })
    .bind(("0.0.0.0", port))
    .unwrap_or_else(|e| panic!("Could not bind to port {}.\n{}", port, e))
    .run()
    .await
    .unwrap_or_else(|e| panic!("Could not run server to port {}.\n{}", port, e));
}
