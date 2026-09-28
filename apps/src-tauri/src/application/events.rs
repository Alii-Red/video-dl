use uuid::Uuid;

use crate::domain::entities::{MediaSource, TaskState};
use crate::domain::value_objects::Progress;

/// Internal domain events published on the in-process bus. Decouples producers
/// (use cases, engine, scheduler) from consumers. Distinct from Tauri IPC events
/// (which are added at the IPC layer in Phase 2).
#[derive(Debug, Clone, PartialEq)]
pub enum DomainEvent {
    /// Carries the fully-probed source (format, qualities, protection) so
    /// consumers (e.g. an interactive page-extraction session) don't need a
    /// second round-trip to learn what was found.
    MediaDetected { source: MediaSource },
    DownloadCreated { download_id: Uuid, task_id: Uuid },
    TaskStateChanged {
        task_id: Uuid,
        state: TaskState,
        /// Set only when `state` is `Failed`: a short human-readable reason
        /// (distilled from yt-dlp's stderr) for the UI.
        error: Option<String>,
    },
    ProgressUpdated { task_id: Uuid, progress: Progress },
    /// The Opera GX extension's active tab/page changed (tab switch or
    /// navigation) — the widget uses this to scope which detections it shows
    /// without discarding older ones (ADR 4.6 follow-up).
    BrowsingContextChanged {
        tab_id: u32,
        page_url: Option<String>,
    },
}
