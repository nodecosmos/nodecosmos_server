#![allow(unused)]
#![feature(const_option)]

use dotenv::dotenv;
use scylla::_macro_internal::ValueList;
use scylla::{CachingSession, Session, SessionBuilder};
use std::time::Duration;
mod db;
mod models;
use chrono::{DateTime, Datelike, Utc};
use scylla::transport::session::TypedRowIter;

use charybdis::prelude::*;
use models::*;

use crate::db::*;

#[tokio::main]
async fn main() {
    dotenv().ok();
    let session = init_session().await;

    let post = Post {
        id: Uuid::new_v4(),
        created_at_day: Utc::now().day() as i32,
        title: "Hello World".into(),
        description: "This is a test".into(),
        created_at: Timestamp::from(Utc::now()),
        updated_at: Timestamp::from(Utc::now()),
    };

    post.insert(&session).await.unwrap();

    // Ops

    let mut post = Post::new();

    post.title = "Hello World".into();
    post.description = "This is a test 2".into();
    post.created_at_day = Utc::now().day() as i32;

    post.delete(&session).await.unwrap();
}
