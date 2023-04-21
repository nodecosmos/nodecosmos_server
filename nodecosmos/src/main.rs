#![feature(async_fn_in_trait)]
#![feature(const_option)]

mod actions;
mod models;
mod nodecosmos;

use actions::*;
use actix_web::{web, App, HttpServer};
use nodecosmos::Nodecosmos;

use crate::nodecosmos::{get_cors, get_db_session, get_port, get_session_middleware};

#[tokio::main]
async fn main() {
    let nodecosmos = Nodecosmos::new();
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
    })
    .bind(("127.0.0.1", port))
    .unwrap_or_else(|e| panic!("Could not bind to port {}.\n{}", port, e))
    .run()
    .await
    .unwrap_or_else(|e| panic!("Could not run server to port {}.\n{}", port, e))
}
