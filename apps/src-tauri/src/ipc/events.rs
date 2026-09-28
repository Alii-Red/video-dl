//! IPC event payloads and channel names. Maps internal [`DomainEvent`]s to the
//! typed payloads the frontend listens for. The actual `emit` bridge (a task that
//! subscribes to the event bus and forwards) is added with the Tauri shell.

use serde::Serialize;
use ts_rs::TS;

use crate::application::events::DomainEvent;

use super::dto::{MediaSourceDto, ProgressDto, TaskStateDto};

/// Tauri event channel names. Kept as constants so Rust and TS agree.
pub mod channel {
    pub const MEDIA_DETECTED: &str = "media://detected";
    pub const DOWNLOAD_CREATED: &str = "download://created";
    pub const TASK_STATE_CHANGED: &str = "task://state-changed";
    pub const PROGRESS: &str = "download://progress";
    pub const BROWSING_CONTEXT_CHANGED: &str = "browsing://context-changed";
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export, export_to = "../frontend/src/types/", rename_all = "camelCase")]
#[serde(rename_all = "camelCase")]
pub struct MediaDetectedPayload {
    pub source: MediaSourceDto,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export, export_to = "../frontend/src/types/", rename_all = "camelCase")]
#[serde(rename_all = "camelCase")]
pub struct DownloadCreatedPayload {
    pub download_id: String,
    pub task_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export, export_to = "../frontend/src/types/", rename_all = "camelCase")]
#[serde(rename_all = "camelCase")]
pub struct TaskStateChangedPayload {
    pub task_id: String,
    pub state: TaskStateDto,
    /// Short failure reason, present only when `state` is `failed`.
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export, export_to = "../frontend/src/types/", rename_all = "camelCase")]
#[serde(rename_all = "camelCase")]
pub struct ProgressPayload {
    pub task_id: String,
    pub progress: ProgressDto,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export, export_to = "../frontend/src/types/", rename_all = "camelCase")]
#[serde(rename_all = "camelCase")]
pub struct BrowsingContextChangedPayload {
    pub tab_id: u32,
    pub page_url: Option<String>,
}

/// A ready-to-emit IPC event: its channel plus its typed payload. The bridge
/// matches on this and calls Tauri `emit(channel, payload)`.
#[derive(Debug, Clone, PartialEq)]
pub enum IpcEvent {
    MediaDetected(MediaDetectedPayload),
    DownloadCreated(DownloadCreatedPayload),
    TaskStateChanged(TaskStateChangedPayload),
    Progress(ProgressPayload),
    BrowsingContextChanged(BrowsingContextChangedPayload),
}

impl IpcEvent {
    pub fn channel(&self) -> &'static str {
        match self {
            IpcEvent::MediaDetected(_) => channel::MEDIA_DETECTED,
            IpcEvent::DownloadCreated(_) => channel::DOWNLOAD_CREATED,
            IpcEvent::TaskStateChanged(_) => channel::TASK_STATE_CHANGED,
            IpcEvent::Progress(_) => channel::PROGRESS,
            IpcEvent::BrowsingContextChanged(_) => channel::BROWSING_CONTEXT_CHANGED,
        }
    }
}

impl From<DomainEvent> for IpcEvent {
    fn from(e: DomainEvent) -> Self {
        match e {
            DomainEvent::MediaDetected { source } => {
                IpcEvent::MediaDetected(MediaDetectedPayload {
                    source: source.into(),
                })
            }
            DomainEvent::DownloadCreated {
                download_id,
                task_id,
            } => IpcEvent::DownloadCreated(DownloadCreatedPayload {
                download_id: download_id.to_string(),
                task_id: task_id.to_string(),
            }),
            DomainEvent::TaskStateChanged {
                task_id,
                state,
                error,
            } => IpcEvent::TaskStateChanged(TaskStateChangedPayload {
                task_id: task_id.to_string(),
                state: state.into(),
                error,
            }),
            DomainEvent::ProgressUpdated { task_id, progress } => {
                IpcEvent::Progress(ProgressPayload {
                    task_id: task_id.to_string(),
                    progress: progress.into(),
                })
            }
            DomainEvent::BrowsingContextChanged { tab_id, page_url } => {
                IpcEvent::BrowsingContextChanged(BrowsingContextChangedPayload {
                    tab_id,
                    page_url,
                })
            }
        }
    }
}
