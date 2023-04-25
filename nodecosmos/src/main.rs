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
                    .service(get_node)
                    .service(create_node)
                    .service(update_node_title)
                    .service(update_node_description)
                    .service(delete_node),
            )
    })
    .bind(("127.0.0.1", port))
    .unwrap_or_else(|e| panic!("Could not bind to port {}.\n{}", port, e))
    .run()
    .await
    .unwrap_or_else(|e| panic!("Could not run server to port {}.\n{}", port, e));
}
