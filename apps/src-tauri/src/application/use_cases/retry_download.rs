use std::sync::Arc;

use uuid::Uuid;

use crate::application::error::ApplicationError;
use crate::application::events::DomainEvent;
use crate::application::ports::{DownloadRepository, EventPublisher};
use crate::domain::entities::TaskState;

/// Re-queue a *failed* download so the scheduler runs it again. Useful for the
/// common transient failures — YouTube's intermittent `403 Forbidden` on
/// fragments, a dropped connection — where simply re-running yt-dlp re-extracts
/// fresh fragment URLs and succeeds. Reuses the existing download row (same
/// URL/format/referer), so nothing else is needed from the caller.
pub struct RetryDownload {
    repo: Arc<dyn DownloadRepository>,
    events: Arc<dyn EventPublisher>,
}

impl RetryDownload {
    pub fn new(repo: Arc<dyn DownloadRepository>, events: Arc<dyn EventPublisher>) -> Self {
        Self { repo, events }
    }

    pub async fn execute(&self, task_id: Uuid) -> Result<(), ApplicationError> {
        let mut task = self.repo.get_task(task_id).await?;
        // Only failed tasks can be retried (the state machine allows
        // Failed→Queued; a completed/cancelled/running task rejects it).
        task.transition_to(TaskState::Queued)?;
        task.error_code = None;
        task.error_message = None;
        self.repo.save_task(task).await?;
        self.events.publish(DomainEvent::TaskStateChanged {
            task_id,
            state: TaskState::Queued,
            error: None,
        });
        Ok(())
    }
}
