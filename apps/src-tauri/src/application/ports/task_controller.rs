use uuid::Uuid;

/// Controls the *running process* of a task (as opposed to its persisted state).
/// Implemented by the scheduler's running-task registry. Lets the cancel/pause
/// use cases actually stop an in-flight yt-dlp process, not just update the DB.
pub trait TaskController: Send + Sync {
    /// Interrupt the task if it is currently running (drops the engine future,
    /// which kills the child process). No-op if the task isn't running.
    fn interrupt(&self, task_id: Uuid);
}
