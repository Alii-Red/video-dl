use std::sync::Arc;

use crate::application::error::ApplicationError;
use crate::application::ports::DownloadRepository;
use crate::domain::entities::{Download, Task};

/// List active (non-terminal) downloads with their task, so the UI can rebuild
/// its downloads panel after a reload.
pub struct ListActiveDownloads {
    repo: Arc<dyn DownloadRepository>,
}

impl ListActiveDownloads {
    pub fn new(repo: Arc<dyn DownloadRepository>) -> Self {
        Self { repo }
    }

    pub async fn execute(&self) -> Result<Vec<(Download, Task)>, ApplicationError> {
        self.repo.list_active().await
    }
}
