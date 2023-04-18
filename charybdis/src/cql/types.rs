#![allow(unused)]

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::fmt::{Debug, Display, Formatter};
use std::net::IpAddr;

pub use scylla::frame::response::result::CqlValue;

pub type Ascii = String;
pub type Boolean = bool;
pub type Blob = Vec<u8>;
pub type Date = i32; // tmp until from_cql_val is implemented for u 32
pub type Double = f64;
pub type Duration = CqlDuration;
pub type Empty = ();
pub type Float = f32;
pub type Int = i32;
pub type BigInt = i64;
pub type Text = String;
pub type Timestamp = chrono::DateTime<Utc>;
pub type Inet = IpAddr;
pub type List<T> = Vec<T>;
pub type Set = Vec<CqlValue>;
pub struct UserDefinedType {
    keyspace: String,
    type_name: String,
    fields: Vec<(String, Option<CqlValue>)>,
}
pub type SmallInt = i16;
pub type TinyInt = i8;
pub type Time = chrono::DateTime<Utc>;
pub type Timeuuid = Uuid;
pub type Tuple = Vec<Option<CqlValue>>;
pub type Varint = BigInt;

pub use scylla::frame::value::{Counter, CqlDuration};
pub use std::collections::HashMap as Map;
pub use uuid::Uuid;
