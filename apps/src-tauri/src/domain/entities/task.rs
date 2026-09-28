use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::errors::DomainError;
use crate::domain::value_objects::Progress;

/// Lifecycle state of a download task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskState {
    Queued,
    Running,
    Paused,
    Completed,
    Failed,
    Cancelled,
}

impl TaskState {
    /// Terminal states cannot transition any further.
    pub fn is_terminal(&self) -> bool {
        matches!(self, TaskState::Completed | TaskState::Cancelled)
    }

    /// The single source of truth for allowed transitions. Keep this exhaustive.
    pub fn can_transition_to(&self, next: TaskState) -> bool {
        use TaskState::*;
        matches!(
            (self, next),
            (Queued, Running)
                | (Queued, Cancelled)
                | (Running, Paused)
                | (Running, Completed)
                | (Running, Failed)
                | (Running, Cancelled)
                // Requeue / crash recovery: a task left Running after an app
                // crash is put back on the queue at boot.
                | (Running, Queued)
                | (Paused, Running)
                | (Paused, Queued)
                | (Paused, Cancelled)
                | (Failed, Queued)
                | (Failed, Cancelled)
        )
    }
}

impl std::fmt::Display for TaskState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            TaskState::Queued => "queued",
            TaskState::Running => "running",
            TaskState::Paused => "paused",
            TaskState::Completed => "completed",
            TaskState::Failed => "failed",
            TaskState::Cancelled => "cancelled",
        };
        write!(f, "{s}")
    }
}

/// The runtime state of a [`super::Download`] — the "where it's at".
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Task {
    pub id: Uuid,
    pub download_id: Uuid,
    pub state: TaskState,
    pub progress: Progress,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Task {
    pub fn new(download_id: Uuid) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            download_id,
            state: TaskState::Queued,
            progress: Progress::zero(),
            error_code: None,
            error_message: None,
            created_at: now,
            updated_at: now,
        }
    }

    /// Move to `next`, enforcing the state machine. Rejects illegal transitions.
    pub fn transition_to(&mut self, next: TaskState) -> Result<(), DomainError> {
        if !self.state.can_transition_to(next) {
            return Err(DomainError::InvalidStateTransition {
                from: self.state.to_string(),
                to: next.to_string(),
            });
        }
        self.state = next;
        self.updated_at = Utc::now();
        Ok(())
    }

    /// Transition to `Failed` and record the error details.
    pub fn fail(
        &mut self,
        code: impl Into<String>,
        message: impl Into<String>,
    ) -> Result<(), DomainError> {
        self.transition_to(TaskState::Failed)?;
        self.error_code = Some(code.into());
        self.error_message = Some(message.into());
        Ok(())
    }

    pub fn update_progress(&mut self, progress: Progress) {
        self.progress = progress;
        self.updated_at = Utc::now();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_happy_path() {
        let mut t = Task::new(Uuid::new_v4());
        assert_eq!(t.state, TaskState::Queued);
        t.transition_to(TaskState::Running).unwrap();
        t.transition_to(TaskState::Paused).unwrap();
        t.transition_to(TaskState::Running).unwrap();
        t.transition_to(TaskState::Completed).unwrap();
        assert!(t.state.is_terminal());
    }

    #[test]
    fn completed_cannot_restart() {
        let mut t = Task::new(Uuid::new_v4());
        t.transition_to(TaskState::Running).unwrap();
        t.transition_to(TaskState::Completed).unwrap();
        assert!(t.transition_to(TaskState::Running).is_err());
    }

    #[test]
    fn illegal_transition_names_states() {
        let mut t = Task::new(Uuid::new_v4());
        let err = t.transition_to(TaskState::Completed).unwrap_err();
        assert_eq!(
            err,
            DomainError::InvalidStateTransition {
                from: "queued".into(),
                to: "completed".into(),
            }
        );
    }

    #[test]
    fn fail_records_details() {
        let mut t = Task::new(Uuid::new_v4());
        t.transition_to(TaskState::Running).unwrap();
        t.fail("network", "connection reset").unwrap();
        assert_eq!(t.state, TaskState::Failed);
        assert_eq!(t.error_code.as_deref(), Some("network"));
    }

    #[test]
    fn failed_can_requeue() {
        let mut t = Task::new(Uuid::new_v4());
        t.transition_to(TaskState::Running).unwrap();
        t.fail("network", "timeout").unwrap();
        assert!(t.transition_to(TaskState::Queued).is_ok());
    }
}
