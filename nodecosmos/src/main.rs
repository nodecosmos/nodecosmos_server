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
    // Example usage:
    partial_user!(PartialUser, id, username, email);

    let mut partial_user = PartialUser {
        id: Uuid::new_v4(),
        username: "test".to_string(),
        email: "test@gmail.com".to_string(),
    };

    println!("{:?}", partial_user);

}
