#![allow(unused)]
#![feature(const_option)]
#![feature(async_fn_in_trait)]

use scylla::{CachingSession, Session, SessionBuilder};
use std::time::Duration;
use dotenv::dotenv;
use scylla::_macro_internal::ValueList;
mod db;
mod models;

use charybdis::prelude::*;
use models::*;

use crate::db::*;

#[tokio::main]
async fn main() {
    let caching_session = init_session().await;
    let user = User {
        id: Uuid::new_v4(),
        ..Default::default()
    }.find_by_primary_key(&caching_session).await.unwrap();
}
