use std::fmt;
use std::error::Error;
use colored::Colorize;
use scylla::transport::errors::{QueryError};
use scylla::transport::query_result::SingleRowTypedError;

#[derive(Debug, Clone)]
pub enum CharybdisError {
    QueryError(QueryError),
    SingleRowTypedError(SingleRowTypedError, String),
    // charybdis
    FromRowError(String),
    NotFoundError(String),
}


impl fmt::Display for CharybdisError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CharybdisError::QueryError(e) => write!(f, "QueryError: {}", e),
            CharybdisError::FromRowError(e) => write!(f, "FromRowError: {}", e),
            CharybdisError::NotFoundError(e) => write!(f, "Records not found for {}", e),
            CharybdisError::SingleRowTypedError(e, model_name) => {
                match e {
                    SingleRowTypedError::RowsExpected(e) => write!(f, "Records not found for {}", model_name),
                    SingleRowTypedError::BadNumberOfRows(e) =>
                        write!(f, "\n\nToo many rows found for find_by_primary_key for {}. {}{}\n\n",
                               model_name, "Expected 1, got: ".red(),
                               e),
                    SingleRowTypedError::FromRowError(e) => write!(f, "DeserializationError: {}", e),
                }
            },
        }
    }
}

impl Error for CharybdisError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            CharybdisError::QueryError(e) => Some(e),
            CharybdisError::FromRowError(_) => None,
            CharybdisError::NotFoundError(_) => None,
            CharybdisError::SingleRowTypedError(e, _) => Some(e),
        }
    }
}
