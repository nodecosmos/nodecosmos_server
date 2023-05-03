#![allow(incomplete_features)]
#![feature(async_fn_in_trait)]
#![feature(const_option)]

mod callbacks;
mod cql;
mod errors;
mod model;
mod operations;
mod query_builder;
mod serializers;

// orm
pub use crate::{
    callbacks::*, cql::types::*, errors::CharybdisError, model::*, operations::*,
    query_builder::CharybdisQuery, query_builder::*, serializers::*,
};

// orm macros
pub use charybdis_macros::{
    char_model_field_attrs_gen, charybdis_model, charybdis_udt_model, charybdis_view_model,
    partial_model_generator,
};

// scylla
pub use scylla::{
    cql_to_rust::{FromCqlVal, FromRow, FromRowError},
    frame::{
        response::result::Row,
        value::{SerializedResult, SerializedValues, ValueList},
    },
    query::Query,
    transport::{errors::QueryError, session::TypedRowIter},
    CachingSession, Session,
};

// scylla macros
pub use scylla::macros::{FromRow, FromUserType, IntoUserType, ValueList};

// additional
pub use serde::{Deserialize, Serialize};
pub use std::collections::HashMap;
