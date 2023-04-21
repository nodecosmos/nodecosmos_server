#![feature(async_fn_in_trait)]
#![feature(const_option)]

mod actions;
mod db;
mod models;

use actions::*;
use actix_web::{web, App, HttpServer};
use db::*;
use scylla::CachingSession;

#[tokio::main]
async fn main() {
    let session: CachingSession = init_session().await;
    let session = web::Data::new(session);

    HttpServer::new(move || {
        App::new().app_data(session.clone()).service(
            web::scope("/users")
                .service(get_user)
                .service(create_user)
                .service(update_user)
                .service(delete_user),
        )
    })
    .bind(("127.0.0.1", 8080))
    .unwrap_or_else(|_| panic!("Could not bind to port {}.", 8080))
    .run()
    .await
    .unwrap_or_else(|_| panic!("Could not run server on port {}.", 8080));
}
