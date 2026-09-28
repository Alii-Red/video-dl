use std::sync::Arc;

use uuid::Uuid;

use crate::application::error::ApplicationError;
use crate::application::ports::DownloadRepository;

/// Remove one entry from the history list. Doesn't touch the file on disk —
/// this is bookkeeping only, not a delete-my-download feature.
pub struct DeleteHistoryEntry {
    repo: Arc<dyn DownloadRepository>,
}

impl DeleteHistoryEntry {
    pub fn new(repo: Arc<dyn DownloadRepository>) -> Self {
        Self { repo }
    }

    pub async fn execute(&self, download_id: Uuid) -> Result<(), ApplicationError> {
        self.repo.delete_history_entry(download_id).await
    }
}
