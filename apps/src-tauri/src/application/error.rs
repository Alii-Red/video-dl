use thiserror::Error;

use crate::domain::errors::DomainError;

/// Errors at the application boundary. Wraps domain errors and infrastructure
/// failures reported through the ports.
#[derive(Debug, Error)]
pub enum ApplicationError {
    #[error(transparent)]
    Domain(#[from] DomainError),

    #[error("not found: {0}")]
    NotFound(String),

    #[error("probe failed: {0}")]
    Probe(String),

    #[error("engine error: {0}")]
    Engine(String),

    #[error("repository error: {0}")]
    Repository(String),
}
