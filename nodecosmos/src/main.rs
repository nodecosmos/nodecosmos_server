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

#[tokio::main]
async fn main() {
    dotenv().ok();
    let session = init_session().await;

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

    posts_vec.to_json();

    println!("elapsed: {:?}", now.elapsed());
}
