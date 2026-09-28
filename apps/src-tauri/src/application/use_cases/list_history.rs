use std::sync::Arc;

use crate::application::error::ApplicationError;
use crate::application::ports::DownloadRepository;
use crate::domain::entities::HistoryEntry;

/// List archived completed downloads (newest first) from the history table.
pub struct ListHistory {
    repo: Arc<dyn DownloadRepository>,
}

impl ListHistory {
    pub fn new(repo: Arc<dyn DownloadRepository>) -> Self {
        Self { repo }
    }

    pub async fn execute(&self, limit: u32) -> Result<Vec<HistoryEntry>, ApplicationError> {
        self.repo.recent_history(limit).await
    }
}
