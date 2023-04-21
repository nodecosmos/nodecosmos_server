#![feature(async_fn_in_trait)]
#![feature(const_option)]

mod actions;
mod models;
mod nodecosmos;

use actions::*;
use actix_cors::Cors;
use actix_web::{http, web, App, HttpServer};
use nodecosmos::Nodecosmos;

#[tokio::main]
async fn main() {
    let nodecosmos = Nodecosmos::new();

    let session = nodecosmos.get_db_session().await;
    let allowed_origin = nodecosmos.get_allowed_origin();

    let session = web::Data::new(session);

    HttpServer::new(move || {
        let cors = Cors::default()
            .allowed_origin(allowed_origin.as_str())
            .allowed_methods(vec!["GET", "POST", "PUT", "DELETE"])
            .allowed_headers(vec![http::header::AUTHORIZATION, http::header::ACCEPT])
            .allowed_header(http::header::CONTENT_TYPE)
            .max_age(3600);

        App::new().wrap(cors).app_data(session.clone()).service(
            web::scope("/users")
                .service(get_user)
                .service(create_user)
                .service(update_user)
                .service(delete_user),
        )
    })
    .bind(("127.0.0.1", 3000))
    .unwrap_or_else(|_| panic!("Could not bind to port {}.", 3000))
    .run()
    .await
    .unwrap_or_else(|_| panic!("Could not run server on port {}.", 3000));
}
