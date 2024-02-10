#![allow(unused_imports)]

mod api;
mod app;
mod constants;
mod errors;
mod models;
mod services;
mod utils;

use crate::models::node::Node;
use actix_web::middleware::Logger;
use actix_web::{web, App as ActixWebApp, HttpServer};
use api::*;
use app::App;
use std::sync::Arc;
use uuid::uuid;
use yrs::updates::decoder::Decode;
use yrs::{Doc, GetString, ReadTxn, Transact, Update};

#[tokio::main]
async fn main() {
    let app = App::new().await;
    let port = app.port();

    app.init().await;
    let app_web_data = web::Data::new(app);

    // web data
    let db_session_web_data = web::Data::from(app_web_data.db_session.clone());
    let elastic_client_web_data = web::Data::from(app_web_data.elastic_client.clone());
    let s3_client_web_data = web::Data::from(app_web_data.s3_client.clone());
    let redis_pool_web_data = web::Data::from(app_web_data.redis_pool.clone());
    let resource_locker_web_data = web::Data::from(app_web_data.resource_locker.clone());
    let desc_ws_conn_pool = web::Data::new(DescriptionWsConnectionPool::default());

    HttpServer::new(move || {
        ActixWebApp::new()
            .wrap(Logger::new("%a %r %s %b %{Referer}i %{User-Agent}i %T"))
            .wrap(app_web_data.cors())
            .wrap(app_web_data.session_middleware())
            .app_data(app_web_data.clone())
            .app_data(db_session_web_data.clone())
            .app_data(elastic_client_web_data.clone())
            .app_data(redis_pool_web_data.clone())
            .app_data(resource_locker_web_data.clone())
            .app_data(s3_client_web_data.clone())
            .app_data(desc_ws_conn_pool.clone())
            .service(web::scope("/ws").service(description_ws))
            .service(
                web::scope("/users")
                    .service(get_user)
                    .service(get_user_by_username)
                    .service(create_user)
                    .service(update_bio)
                    .service(delete_user)
                    .service(login)
                    .service(sync)
                    .service(logout)
                    .service(update_profile_image)
                    .service(delete_profile_image),
            )
            .service(
                web::scope("/nodes")
                    .service(get_nodes)
                    .service(get_node)
                    .service(get_branched_node)
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
                    .service(user_likes)
                    .service(get_like_count)
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
                    .service(delete_contribution_request)
                    .service(publish)
                    .service(merge_contribution_request),
            )
            .service(
                web::scope("attachments")
                    .service(upload_image)
                    .service(get_presigned_url)
                    .service(create_attachment),
            )
            .service(web::scope("branches").service(restore_node))
    })
    .bind(("0.0.0.0", port))
    .unwrap_or_else(|e| panic!("Could not bind to port {}.\n{}", port, e))
    .run()
    .await
    .unwrap_or_else(|e| panic!("Could not run server to port {}.\n{}", port, e));
}
