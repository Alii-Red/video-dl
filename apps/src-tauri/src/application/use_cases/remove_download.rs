use std::sync::Arc;

use uuid::Uuid;

use crate::application::error::ApplicationError;
use crate::application::ports::DownloadRepository;
use crate::domain::entities::TaskState;
use crate::domain::errors::DomainError;

/// Permanently dismiss a download that's done running (failed or cancelled).
/// `list_active` only excludes `completed`/`cancelled`, not `failed` — without
/// this, a failed entry would linger in the queue view forever, and "Retirer"
/// would otherwise only clear it from the frontend store (it'd reappear on
/// the next reload/hydration from the backend).
pub struct RemoveDownload {
    repo: Arc<dyn DownloadRepository>,
}

impl RemoveDownload {
    pub fn new(repo: Arc<dyn DownloadRepository>) -> Self {
        Self { repo }
    }

    pub async fn execute(&self, task_id: Uuid) -> Result<(), ApplicationError> {
        let task = self.repo.get_task(task_id).await?;
        if !matches!(task.state, TaskState::Failed | TaskState::Cancelled) {
            return Err(ApplicationError::Domain(DomainError::InvalidOperation(
                "seul un téléchargement échoué ou annulé peut être retiré".into(),
            )));
        }
        self.repo.delete_download(task.download_id).await
    }
}
