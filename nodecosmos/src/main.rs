#![feature(async_fn_in_trait)]
#![allow(unused)]
#![feature(const_option)]

use dotenv::dotenv;
use scylla::_macro_internal::ValueList;
use scylla::{CachingSession, Session, SessionBuilder};
use std::time::Duration;
mod db;
mod models;
use chrono::{DateTime, Datelike, Local, Utc};
use scylla::transport::session::TypedRowIter;

use charybdis::prelude::*;
use models::*;

use crate::db::*;

use actix_web::{get, post, web, App, HttpResponse, HttpServer, Responder};
use scylla::batch::Batch;

#[get("/")]
async fn hello() -> impl Responder {
    HttpResponse::Ok().body("Hello world!")
}

#[post("/echo")]
async fn echo(req_body: String) -> impl Responder {
    HttpResponse::Ok().body(req_body)
}

async fn manual_hello(session: web::Data<CachingSession>) -> impl Responder {
    let posts = Post::find(
        &session,
        find_post_query!("created_at_day = ?"),
        (Utc::now().date_naive(),),
    )
    .await
    .unwrap();
    let now = std::time::Instant::now();
    let mut posts_vec = vec![];

    for post in posts {
        posts_vec.push(post.unwrap());
    }

    HttpResponse::Ok().json(posts_vec)
}

#[tokio::main]
async fn main() {
    let session: CachingSession = init_session().await;
    // let session = web::Data::new(session);
    //
    // HttpServer::new(move || {
    //     App::new()
    //         .service(hello)
    //         .service(echo)
    //         .app_data(session.clone())
    //         .route("/hey.json", web::get().to(manual_hello))
    // })
    // .bind(("127.0.0.1", 8080))?
    // .run()
    // .await;
    //
    let user = User::new();
    let res = user.insert_cb(&session).await;
    match res {
        Ok(_) => println!("success"),
        Err(e) => match e {
            CharybdisError::ValidationError((field, reason)) => {
                println!("validation error: {} {}", field, reason)
            }
            _ => println!("error: {:?}", e),
        },
    }

    let id = Uuid::new_v4();
    let mut user = User {
        id,
        username: "initial_username".to_string(),
        email: "test@nodecosmos.com".to_string(),
        password: "test".to_string(),
        hashed_password: "test".to_string(),
        first_name: "test".to_string(),
        last_name: "test".to_string(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
        address: None,
    };

    partial_user!(PrtUser, id, username);

    let mut user2 = PrtUser {
        id,
        username: "updated_username".to_string(),
    };

    let mut batch: Batch = Default::default();

    batch.append_statement(User::INSERT_QUERY);
    batch.append_statement(PrtUser::UPDATE_QUERY);

    let batch_res = session
        .batch(&batch, (&user, &user2.get_update_values()))
        .await
        .unwrap();
    let user = user.find_by_primary_key(&session).await.unwrap();

    assert_eq!(user.username, "updated_username".to_string());
}
