use std::fmt;
use std::error::Error;
use scylla::transport::errors::{QueryError};
use scylla::transport::query_result::{RowsExpectedError};

#[derive(Debug, Clone)]
pub enum CharybdisError {
    QueryError(QueryError),
    NotFoundError(String),
}


impl fmt::Display for CharybdisError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CharybdisError::QueryError(e) => write!(f, "QueryError: {}", e),
            CharybdisError::NotFoundError(e) => write!(f, "Records not found for {}", e),
        }
    }
}

impl Error for CharybdisError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            CharybdisError::QueryError(e) => Some(e),
            CharybdisError::NotFoundError(_) => None,
        }
    }
}
