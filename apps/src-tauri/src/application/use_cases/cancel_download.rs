use std::sync::Arc;

use uuid::Uuid;

use crate::application::error::ApplicationError;
use crate::application::events::DomainEvent;
use crate::application::ports::{DownloadRepository, EventPublisher, TaskController};
use crate::domain::entities::TaskState;

/// Cancel a task: mark it Cancelled (enforcing the state machine) and interrupt
/// its running process, if any.
pub struct CancelDownload {
    repo: Arc<dyn DownloadRepository>,
    events: Arc<dyn EventPublisher>,
    controller: Arc<dyn TaskController>,
}

impl CancelDownload {
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

    pub async fn execute(&self, task_id: Uuid) -> Result<(), ApplicationError> {
        let mut task = self.repo.get_task(task_id).await?;
        task.transition_to(TaskState::Cancelled)?;
        self.repo.save_task(task).await?;
        // Set state first, then stop the process: the scheduler sees the token
        // fire and skips finalizing, leaving the Cancelled state in place.
        self.controller.interrupt(task_id);
        self.events.publish(DomainEvent::TaskStateChanged {
            task_id,
            state: TaskState::Cancelled,
            error: None,
        });
        Ok(())
    }
}
