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
    partial_user!(OpsUser, id, username);

    let mut ops_user: OpsUser = OpsUser::new();
    ops_user.id = Uuid::new_v4();

    let session: &CachingSession = init_session().await;
    let id: Uuid = Uuid::new_v4();
    let user: User = User::new();

    let res = user.insert(&session).await;

    let user: User = user.find_by_primary_key(&session).await.unwrap();

    println!("{:?}", user);

    println!("{:?}", user.to_json());

    let users: TypedRowIter<User> = user.find_by_partition_key(&session).await.unwrap();

    let created_at = chrono::Utc::now().day().to_string();
    let updated_at = chrono::Utc::now();

    let user: User = User::new();

    let query = find_user_query!("email = ? ALLOW FILTERING");
    let email = "test_email";

    let users: TypedRowIter<User> = User::find(&session, query, (email,)).await.unwrap();

    for user in users {
        println!("{:?}", user);
    }

    let mut user_by_username: UsersByUsername = UsersByUsername::new();
    user_by_username.username = "test_username".to_string();

    let users_by_username: TypedRowIter<UsersByUsername> = user_by_username
        .find_by_partition_key(&session)
        .await
        .unwrap();

    for user in users_by_username {
        println!("{:?}", user);
    }

    let mut users_by_username: UsersByUsername = UsersByUsername::new();

    let query = find_users_by_username_query!("username = ?");

    let users_by_username: TypedRowIter<UsersByUsername> =
        UsersByUsername::find(&session, query, ("test_username",))
            .await
            .unwrap();

    for user in users_by_username {
        println!("{:?}", user);
    }
}
