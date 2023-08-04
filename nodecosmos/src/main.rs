#![allow(incomplete_features)]
#![feature(async_fn_in_trait)]
#![feature(const_option)]

mod actions;
mod app;
mod authorize;
mod elastic;
mod errors;
mod models;
mod services;

use crate::app::{
    get_cors, get_db_session, get_elastic_client, get_port, get_redis_pool, get_session_middleware,
    CbExtension,
};
use crate::services::resource_locker::ResourceLocker;
use actions::*;
use actix_web::middleware::Logger;
use actix_web::{web, App, HttpServer};
use app::App as NodecosmosApp;
use deadpool_redis::Pool;
use std::sync::Arc;

#[tokio::main]
async fn main() {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    let nodecosmos = NodecosmosApp::new();
    let db_session = get_db_session(&nodecosmos).await;
    let elastic_client = get_elastic_client(&nodecosmos).await;
    let port = get_port(&nodecosmos);
    let cb_extension = CbExtension {
        elastic_client: elastic_client.clone(),
    };

    // web data
    let db_session_web_data = web::Data::new(db_session);
    let elastic_client_web_data = web::Data::new(elastic_client.clone());
    let cb_extension_web_data = web::Data::new(cb_extension.clone());

    let pool: Pool = get_redis_pool(&nodecosmos).await;
    let pool_web_data = web::Data::new(pool.clone());

    elastic::build(&elastic_client).await;

    HttpServer::new(move || {
        let pool_arc = Arc::clone(&pool_web_data);

        let resource_locker = ResourceLocker::new(pool_arc);
        let resource_locker_web_data = web::Data::new(resource_locker);

        App::new()
            .wrap(Logger::new("%a %r %s %b %{Referer}i %{User-Agent}i %T"))
            .wrap(get_cors(&nodecosmos))
            .wrap(get_session_middleware(&nodecosmos))
            .app_data(db_session_web_data.clone())
            .app_data(elastic_client_web_data.clone())
            .app_data(cb_extension_web_data.clone())
            .app_data(pool_web_data.clone())
            .app_data(resource_locker_web_data)
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
                    .service(get_root_node)
                    .service(get_node)
                    .service(create_node)
                    .service(update_node_title)
                    .service(update_node_description)
                    .service(delete_node)
                    .service(get_node_description)
                    .service(reorder_nodes),
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
                    .service(update_initial_inputs),
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
                    .service(update_contribution_request_description),
            )
            .service(
                web::scope("commits")
                    .service(create_node_commit)
                    .service(update_node_commit_title)
                    .service(update_node_commit_description)
                    .service(delete_node_commit)
                    .service(create_workflow_commit),
            )
    })
    .bind(("0.0.0.0", port))
    .unwrap_or_else(|e| panic!("Could not bind to port {}.\n{}", port, e))
    .run()
    .await
    .unwrap_or_else(|e| panic!("Could not run server to port {}.\n{}", port, e));
}
