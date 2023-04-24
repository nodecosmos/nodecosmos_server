use colored::Colorize;
use scylla::frame::value::SerializeValuesError;
use scylla::transport::errors::QueryError;
use scylla::transport::query_result::{RowsExpectedError, SingleRowTypedError};
use std::error::Error;
use std::fmt;

#[derive(Debug, Clone)]
pub enum CharybdisError {
    // scylla
    QueryError(QueryError),
    RowsExpectedError(RowsExpectedError),
    SingleRowTypedError(SingleRowTypedError, String),
    SerializeValuesError(SerializeValuesError, String),
    // charybdis
    NotFoundError(String),
    ValidationError((String, String)),
    CustomError(String),
}

impl fmt::Display for CharybdisError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            // scylla errors
            CharybdisError::QueryError(e) => write!(f, "QueryError: {}", e),
            CharybdisError::RowsExpectedError(e) => write!(f, "RowsExpectedError: {:?}", e),
            CharybdisError::SingleRowTypedError(e, model_name) => match e {
                SingleRowTypedError::RowsExpected(_) => {
                    write!(f, "Records not found for {}", model_name)
                }
                SingleRowTypedError::BadNumberOfRows(e) => write!(
                    f,
                    "\n\nBad number of rows for find_by_primary_key for {}. {}{}\n\n",
                    model_name,
                    "Expected 1, got: ".red(),
                    e
                ),
                SingleRowTypedError::FromRowError(e) => write!(f, "DeserializationError: {}", e),
            },
            CharybdisError::SerializeValuesError(e, model) => {
                write!(f, "SerializeValuesError: {}\n{}", e, model)
            }
            // charybdis
            CharybdisError::NotFoundError(e) => write!(f, "Records not found for query: {}", e),
            CharybdisError::ValidationError(e) => write!(f, "ValidationError: {} {}", e.0, e.1),
            CharybdisError::CustomError(e) => write!(f, "CustomError: {}", e),
        }
    }
}

impl Error for CharybdisError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            CharybdisError::QueryError(e) => Some(e),
            CharybdisError::RowsExpectedError(e) => Some(e),
            CharybdisError::NotFoundError(_) => None,
            CharybdisError::SingleRowTypedError(e, _) => Some(e),
            CharybdisError::SerializeValuesError(e, _) => Some(e),
            CharybdisError::ValidationError(_) => None,
            CharybdisError::CustomError(_) => None,
        }
    }
}

impl From<QueryError> for CharybdisError {
    fn from(e: QueryError) -> Self {
        CharybdisError::QueryError(e)
    }
}

impl From<RowsExpectedError> for CharybdisError {
    fn from(e: RowsExpectedError) -> Self {
        CharybdisError::RowsExpectedError(e)
    }
}

impl From<SingleRowTypedError> for CharybdisError {
    fn from(e: SingleRowTypedError) -> Self {
        CharybdisError::SingleRowTypedError(e, "model".to_string())
    }
}
