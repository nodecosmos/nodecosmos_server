#![allow(incomplete_features)]
#![feature(async_fn_in_trait)]
#![feature(const_option)]

mod actions;
mod app;
mod authorize;
mod errors;
mod models;

use actions::*;
// use actix_web::middleware::Logger;
use actix_web::{web, App, HttpServer};
use app::App as NodecosmosApp;

use crate::app::{get_cors, get_db_session, get_port, get_session_middleware};

#[tokio::main]
async fn main() {
    // env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    let nodecosmos = NodecosmosApp::new();
    let session = get_db_session(&nodecosmos).await;
    let port = get_port(&nodecosmos);

    HttpServer::new(move || {
        App::new()
            // .wrap(Logger::new("%a %r %s %b %{Referer}i %{User-Agent}i %T"))
            .wrap(get_cors(&nodecosmos))
            .wrap(get_session_middleware(&nodecosmos))
            .app_data(session.clone())
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
                    .service(delete_node),
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
                    .service(create_workflow),
            )
            .service(
                web::scope("/flows")
                    .service(create_flow)
                    .service(update_flow)
                    .service(delete_flow),
            )
            .service(
                web::scope("/flow_steps")
                    .service(create_flow_step)
                    .service(update_flow_step)
                    .service(delete_flow_step),
            )
            .service(web::scope("input_outputs").service(create_io))
    })
    .bind(("127.0.0.1", port))
    .unwrap_or_else(|e| panic!("Could not bind to port {}.\n{}", port, e))
    .run()
    .await
    .unwrap_or_else(|e| panic!("Could not run server to port {}.\n{}", port, e));
}
