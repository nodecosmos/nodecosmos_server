use scylla::transport::errors::{QueryError};
use scylla::transport::query_result::{RowsExpectedError};

pub enum CharybdisError {
    QueryError(QueryError),
    RowsExpectedError(RowsExpectedError),
}