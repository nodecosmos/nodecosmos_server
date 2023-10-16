#![allow(incomplete_features)]
#![feature(async_fn_in_trait)]

pub mod batch;
pub mod callbacks;
pub mod errors;
pub mod iterator;
pub mod model;
pub mod operations;
pub mod serializers;
pub mod stream;
pub mod types;

// orm macros
pub mod macros {
    pub use charybdis_macros::{
        char_model_field_attrs_gen, charybdis_model, charybdis_udt_model, charybdis_view_model,
    };
}

// scylla
// TODO: Lets reorganize this according to: https://github.com/scylladb/scylla-rust-driver/pull/817
pub use scylla::{
    cql_to_rust::{FromCqlVal, FromRow, FromRowError},
    frame::{
        response::result::Row,
        value::{SerializedResult, SerializedValues, ValueList},
    },
    query::Query,
    transport::{errors::QueryError, session::TypedRowIter},
    CachingSession, QueryResult, Session,
};

// scylla macros
pub use scylla::macros::{FromRow, FromUserType, IntoUserType, ValueList};
