use serde::Serialize;
use ts_rs::TS;

use crate::application::error::ApplicationError;

/// Error shape crossing the IPC boundary. Serializable and mirrored to TypeScript.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../frontend/src/types/", rename_all = "camelCase")]
#[serde(rename_all = "camelCase")]
pub struct IpcError {
    pub kind: IpcErrorKind,
    pub message: String,
}

#[derive(Debug, Clone, Copy, Serialize, TS)]
#[ts(export, export_to = "../frontend/src/types/", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum IpcErrorKind {
    Validation,
    NotFound,
    Probe,
    Engine,
    Repository,
}

impl IpcError {
    pub fn new(kind: IpcErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

impl From<ApplicationError> for IpcError {
    fn from(e: ApplicationError) -> Self {
        let kind = match &e {
            ApplicationError::Domain(_) => IpcErrorKind::Validation,
            ApplicationError::NotFound(_) => IpcErrorKind::NotFound,
            ApplicationError::Probe(_) => IpcErrorKind::Probe,
            ApplicationError::Engine(_) => IpcErrorKind::Engine,
            ApplicationError::Repository(_) => IpcErrorKind::Repository,
        };
        IpcError::new(kind, e.to_string())
    }
}
