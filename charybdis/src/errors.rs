use colored::Colorize;
use scylla::frame::value::SerializeValuesError;
use scylla::transport::errors::QueryError;
use scylla::transport::query_result::{
    FirstRowTypedError, MaybeFirstRowTypedError, RowsExpectedError, SingleRowTypedError,
};
use std::error::Error;
use std::fmt;

#[derive(Debug, Clone)]
pub enum CharybdisError {
    // scylla
    QueryError(QueryError),
    RowsExpectedError(RowsExpectedError, String),
    SingleRowTypedError(SingleRowTypedError, String),
    SerializeValuesError(SerializeValuesError, String),
    FirstRowTypedError(FirstRowTypedError, String),
    MaybeFirstRowTypedError(MaybeFirstRowTypedError, String),
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
            CharybdisError::RowsExpectedError(e, model_name) => {
                write!(f, "RowsExpectedError: {:?} \nin Mode: {}", e, model_name)
            }
            CharybdisError::SingleRowTypedError(e, model_name) => write!(
                f,
                "SingleRowTypedError: {:?} \n in Model: {}",
                e, model_name
            ),
            CharybdisError::FirstRowTypedError(e, model) => {
                write!(f, "FirstRowTypedError: {:?} \nin Model: {}", e, model)
            }
            CharybdisError::MaybeFirstRowTypedError(e, model) => {
                write!(f, "FirstRowTypedError: {:?} \nin Model: {}", e, model)
            }
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
            CharybdisError::RowsExpectedError(e, _) => Some(e),
            CharybdisError::NotFoundError(_) => None,
            CharybdisError::SingleRowTypedError(e, _) => Some(e),
            CharybdisError::FirstRowTypedError(e, _) => Some(e),
            CharybdisError::MaybeFirstRowTypedError(e, _) => Some(e),
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
        CharybdisError::RowsExpectedError(e, "unknown".to_string())
    }
}

impl From<SingleRowTypedError> for CharybdisError {
    fn from(e: SingleRowTypedError) -> Self {
        CharybdisError::SingleRowTypedError(e, "unknown".to_string())
    }
}
