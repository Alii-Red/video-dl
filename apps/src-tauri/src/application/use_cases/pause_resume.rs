use std::sync::Arc;

use uuid::Uuid;

use crate::application::error::ApplicationError;
use crate::application::events::DomainEvent;
use crate::application::ports::{DownloadRepository, EventPublisher, TaskController};
use crate::domain::entities::TaskState;

/// Pause and resume a task.
///
/// Pause marks the task Paused and interrupts its running process; yt-dlp keeps
/// its `.part` files. Resume re-queues it so the scheduler relaunches yt-dlp with
/// `--continue` (the caller re-dispatches queued work).
pub struct PauseResume {
    repo: Arc<dyn DownloadRepository>,
    events: Arc<dyn EventPublisher>,
    controller: Arc<dyn TaskController>,
}

impl PauseResume {
    pub fn new(
        repo: Arc<dyn DownloadRepository>,
        events: Arc<dyn EventPublisher>,
        controller: Arc<dyn TaskController>,
    ) -> Self {
        Self {
            repo,
            events,
            controller,
        }
    }

    pub async fn pause(&self, task_id: Uuid) -> Result<(), ApplicationError> {
        let mut task = self.repo.get_task(task_id).await?;
        task.transition_to(TaskState::Paused)?;
        self.repo.save_task(task).await?;
        self.controller.interrupt(task_id);
        self.events.publish(DomainEvent::TaskStateChanged {
            task_id,
            state: TaskState::Paused,
            error: None,
        });
        Ok(())
    }

    pub async fn resume(&self, task_id: Uuid) -> Result<(), ApplicationError> {
        let mut task = self.repo.get_task(task_id).await?;
        // Back to the queue; the scheduler picks it up and continues the download.
        task.transition_to(TaskState::Queued)?;
        self.repo.save_task(task).await?;
        self.events.publish(DomainEvent::TaskStateChanged {
            task_id,
            state: TaskState::Queued,
            error: None,
        });
        Ok(())
    }
}
