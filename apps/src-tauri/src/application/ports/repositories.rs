use async_trait::async_trait;
use uuid::Uuid;

use crate::application::error::ApplicationError;
use crate::domain::entities::{Download, HistoryEntry, Task, TaskState};

/// Persistence port for downloads and their tasks. Implemented in-memory in
/// Phase 1, by SQLx in Phase 3.
#[async_trait]
pub trait DownloadRepository: Send + Sync {
    async fn create(&self, download: Download, task: Task) -> Result<(), ApplicationError>;

    async fn get_download(&self, id: Uuid) -> Result<Download, ApplicationError>;

    /// Persist changes to an existing download (e.g. its `output_path`).
    async fn save_download(&self, download: Download) -> Result<(), ApplicationError>;

    async fn get_task(&self, id: Uuid) -> Result<Task, ApplicationError>;

    async fn save_task(&self, task: Task) -> Result<(), ApplicationError>;

    async fn list_tasks_by_state(
        &self,
        state: TaskState,
    ) -> Result<Vec<Task>, ApplicationError>;

    async fn list_all_tasks(&self) -> Result<Vec<Task>, ApplicationError>;

    /// Non-terminal downloads (queued/running/paused/failed) joined with their
    /// task — powers the active downloads view (survives a UI reload).
    async fn list_active(&self) -> Result<Vec<(Download, Task)>, ApplicationError>;

    /// Archive a completed download in the history table.
    async fn record_history(
        &self,
        download_id: Uuid,
        final_path: Option<String>,
        bytes: Option<u64>,
    ) -> Result<(), ApplicationError>;

    /// Most recent history entries (newest first).
    async fn recent_history(&self, limit: u32) -> Result<Vec<HistoryEntry>, ApplicationError>;

    /// Permanently remove a download (and its task, via cascade) — used to
    /// dismiss a failed/cancelled entry that would otherwise linger in the
    /// active list forever (`list_active` only excludes completed/cancelled
    /// states, not failed).
    async fn delete_download(&self, id: Uuid) -> Result<(), ApplicationError>;

    /// Remove one archived entry from history (doesn't touch the file on disk).
    async fn delete_history_entry(&self, download_id: Uuid) -> Result<(), ApplicationError>;
}
