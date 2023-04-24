use std::error::Error;
use std::fmt;

#[derive(Debug, Clone)]
pub enum NodecosmosError {
    ClientSessionError(String),
    AuthError(String),
}

impl fmt::Display for NodecosmosError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NodecosmosError::ClientSessionError(e) => write!(f, "Session Error: {}", e),
            NodecosmosError::AuthError(e) => write!(f, "Auth Error: {}", e),
        }
    }
}

impl Error for NodecosmosError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            NodecosmosError::ClientSessionError(_) => None,
            NodecosmosError::AuthError(_) => None,
        }
    }
}
