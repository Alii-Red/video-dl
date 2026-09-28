use std::sync::Arc;

use crate::application::error::ApplicationError;
use crate::application::events::DomainEvent;
use crate::application::ports::{DownloadRepository, EventPublisher};
use crate::domain::entities::TaskState;

/// Boot-time recovery: any task still marked `Running` in the database was
/// interrupted by an app crash. Put it back on the queue so the scheduler can
/// resume it (yt-dlp `--continue`).
pub struct RecoverInterruptedTasks {
    repo: Arc<dyn DownloadRepository>,
    events: Arc<dyn EventPublisher>,
}

impl RecoverInterruptedTasks {
    pub fn new(repo: Arc<dyn DownloadRepository>, events: Arc<dyn EventPublisher>) -> Self {
        Self { repo, events }
    }

    /// Returns the number of tasks re-queued.
    pub async fn execute(&self) -> Result<usize, ApplicationError> {
        let interrupted = self.repo.list_tasks_by_state(TaskState::Running).await?;
        let mut count = 0;
        for mut task in interrupted {
            task.transition_to(TaskState::Queued)?;
            let task_id = task.id;
            self.repo.save_task(task).await?;
            self.events.publish(DomainEvent::TaskStateChanged {
                task_id,
                state: TaskState::Queued,
                error: None,
            });
            count += 1;
        }
        Ok(count)
    }
}
