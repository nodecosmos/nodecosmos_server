#![allow(unused)]
#![feature(const_option)]

use scylla::{CachingSession, Session, SessionBuilder};
use std::time::Duration;
use dotenv::dotenv;
use scylla::_macro_internal::ValueList;
mod db;
mod models;
use chrono::{DateTime, Utc};

use charybdis::prelude::*;
use models::*;

use crate::db::*;



#[tokio::main]
async fn main() {
    // partial_user!(OpsUser, id, username);
    //
    // let json = r#"
    //
    // "#;
    //
    // let user: OpsUser = OpsUser::from_json(json);

    let session: &CachingSession = init_session().await;

    let id: Uuid = Uuid::new_v4();

    let user: User = User {
        id,
        email: "test_email".to_string(),
        username: "test_username".to_string(),
        password: "test_pass".to_string(),
        hashed_password: "test_hashed_pass".to_string(),
        created_at: DateTime::from(Utc::now()),
        updated_at: DateTime::from(Utc::now()),
        address: Address {
            street: "street".to_string(),
            state: "state".to_string(),
            zip: "zip".to_string(),
            country: "country".to_string(),
            city: "city".to_string(),
        },
    };

    let res = user.insert(&session).await;

    let mut user = User::new();
    user.id = id;
    let res: User = user.find_by_primary_key(&session).await.unwrap();

    println!("{:?}", res);

    let new_id = Uuid::new_v4();

    partial_user!(OpsUser, id, username);

    let user: OpsUser = OpsUser {
        id: new_id,
        username: "test_ops_user_username".to_string(),
    };

    user.insert(&session).await;

    let res: OpsUser = user.find_by_primary_key(&session).await.unwrap();

    println!("{:?}", res);

    let user = User {id, ..Default::default()};
    let mut user: User = user.find_by_primary_key(&session).await.unwrap();

    println!("{:?}", user);

    user.username = "charybdis_username".to_string();
    user.email = "goran@nodecosmos.com".to_string();

    user.update(&session).await.unwrap();

    let res: User = user.find_by_primary_key(&session).await.unwrap();

    println!("{:?}", res);

    let user_json: &str = r#"
        {
            "email": "test_email",
            "username": "username"
        }
    "#;


    let mut user = User::from_json(user_json);
    user.id = Uuid::new_v4();
    user.insert(&session).await;

    user = user.find_by_primary_key(&session).await.unwrap();

    println!("{:?}", user);
}
