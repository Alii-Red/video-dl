use thiserror::Error;

/// Errors raised by pure domain rules. No I/O concerns here.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum DomainError {
    #[error("invalid URL: {0}")]
    InvalidUrl(String),

    #[error("unsupported media format")]
    UnsupportedFormat,

    #[error("invalid state transition from {from} to {to}")]
    InvalidStateTransition { from: String, to: String },

    #[error("no quality available")]
    NoQualityAvailable,

    #[error("{0}")]
    InvalidOperation(String),
}
