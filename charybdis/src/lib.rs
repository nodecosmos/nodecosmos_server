#![feature(async_fn_in_trait)]
#![feature(const_option)]

mod cql;
mod operations;
mod serializers;
mod model;

pub(crate) mod errors;

// all the public stuff
pub mod prelude;
mod query_builder;
