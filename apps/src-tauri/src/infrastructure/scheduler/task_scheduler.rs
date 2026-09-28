use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{mpsc, Semaphore};
use uuid::Uuid;

use crate::application::error::ApplicationError;
use crate::application::events::DomainEvent;
use crate::domain::value_objects::Progress;

/// How often at most a running task's progress is written to SQLite. UI
/// events are unthrottled; only the DB write is rate-limited.
const PROGRESS_PERSIST_INTERVAL: Duration = Duration::from_millis(1000);
use crate::application::ports::{
    DownloadEngine, DownloadPlanner, DownloadRepository, EngineOutcome, EventPublisher,
};
use crate::domain::entities::TaskState;

use super::running::RunningTasks;

/// Runs queued tasks through the engine, respecting a concurrency limit.
/// Cheaply cloneable (all fields are `Arc`) so each task runs in its own
/// `tokio::task` with an owned semaphore permit.
#[derive(Clone)]
pub struct TaskScheduler {
    repo: Arc<dyn DownloadRepository>,
    engine: Arc<dyn DownloadEngine>,
    planner: Arc<dyn DownloadPlanner>,
    events: Arc<dyn EventPublisher>,
    semaphore: Arc<Semaphore>,
    running: RunningTasks,
}

impl TaskScheduler {
    pub fn new(
        repo: Arc<dyn DownloadRepository>,
        engine: Arc<dyn DownloadEngine>,
        planner: Arc<dyn DownloadPlanner>,
        events: Arc<dyn EventPublisher>,
        concurrency: usize,
        running: RunningTasks,
    ) -> Self {
        Self {
            repo,
            engine,
            planner,
            events,
            semaphore: Arc::new(Semaphore::new(concurrency.max(1))),
            running,
        }
    }

    /// Dispatch every currently-queued task, bounded by the concurrency limit,
    /// and wait for them all to finish.
    pub async fn run_pending(&self) -> Result<(), ApplicationError> {
        let queued = self.repo.list_tasks_by_state(TaskState::Queued).await?;
        let mut handles = Vec::with_capacity(queued.len());

        for task in queued {
            // Acquire before spawning so at most `concurrency` run at once.
            let permit = self
                .semaphore
                .clone()
                .acquire_owned()
                .await
                .map_err(|e| ApplicationError::Engine(e.to_string()))?;
            let scheduler = self.clone();
            handles.push(tokio::spawn(async move {
                let _permit = permit;
                let _ = scheduler.run_task(task.id).await;
            }));
        }

        for handle in handles {
            let _ = handle.await;
        }
        Ok(())
    }

    /// Run a single task to completion: Queued → Running → (Completed | Failed),
    /// streaming progress to the repository and the event bus.
    #[tracing::instrument(skip(self), fields(%task_id))]
    pub async fn run_task(&self, task_id: Uuid) -> Result<(), ApplicationError> {
        let mut task = self.repo.get_task(task_id).await?;
        let download = self.repo.get_download(task.download_id).await?;

        task.transition_to(TaskState::Running)?;
        self.repo.save_task(task).await?;
        self.publish_state(task_id, TaskState::Running);
        tracing::info!(url = %download.source_url.as_str(), "download started");

        let request = self.planner.plan(&download).await?;

        // Forward engine progress to persistence + the event bus. The UI event
        // fires on EVERY tick (smooth progress bar), but persistence is
        // throttled: yt-dlp emits progress many times per second, and a
        // get+save round-trip to SQLite on each one is pure I/O waste — the
        // DB row only matters for surviving a crash/reload, for which
        // ~once-per-second granularity is plenty. The final tick is always
        // flushed so the last known percent isn't lost.
        let (tx, mut rx) = mpsc::channel(32);
        let repo = self.repo.clone();
        let events = self.events.clone();
        let drain = tokio::spawn(async move {
            let mut last_persist = tokio::time::Instant::now();
            let mut pending: Option<Progress> = None;
            while let Some(progress) = rx.recv().await {
                events.publish(DomainEvent::ProgressUpdated { task_id, progress });
                pending = Some(progress);
                if last_persist.elapsed() >= PROGRESS_PERSIST_INTERVAL {
                    if let Ok(mut t) = repo.get_task(task_id).await {
                        t.update_progress(progress);
                        let _ = repo.save_task(t).await;
                    }
                    last_persist = tokio::time::Instant::now();
                    pending = None;
                }
            }
            // Flush the last value the throttle skipped.
            if let Some(progress) = pending {
                if let Ok(mut t) = repo.get_task(task_id).await {
                    t.update_progress(progress);
                    let _ = repo.save_task(t).await;
                }
            }
        });

        // Register a cancellation token so cancel/pause can interrupt the run:
        // when the token fires, the `engine.run` future is dropped, and the
        // engine's `kill_on_drop` terminates the yt-dlp child.
        let token = self.running.register(task_id);
        let outcome = tokio::select! {
            biased;
            _ = token.cancelled() => None,
            res = self.engine.run(request, tx) => Some(res),
        };
        self.running.finish(task_id);
        let _ = drain.await;

        match outcome {
            // Interrupted by cancel/pause: the use case already set the terminal
            // (or Paused) state, so don't finalize here.
            None => {
                tracing::info!("download interrupted (cancel/pause)");
                Ok(())
            }
            Some(Ok(outcome)) => {
                tracing::info!("download completed");
                self.complete(task_id, outcome).await?;
                Ok(())
            }
            Some(Err(e)) => {
                tracing::warn!(error = %e, "download failed");
                self.finalize(task_id, TaskState::Failed, Some(e.to_string()))
                    .await?;
                Err(e)
            }
        }
    }

    /// Mark the task Completed, persist its output path, and archive it in
    /// history. Skips if the task was cancelled/changed while running.
    async fn complete(
        &self,
        task_id: Uuid,
        outcome: EngineOutcome,
    ) -> Result<(), ApplicationError> {
        let mut task = self.repo.get_task(task_id).await?;
        if task.state != TaskState::Running {
            return Ok(());
        }
        task.transition_to(TaskState::Completed)?;
        let download_id = task.download_id;
        self.repo.save_task(task).await?;
        self.publish_state(task_id, TaskState::Completed);

        if let Some(path) = outcome.output_path.clone() {
            if let Ok(mut download) = self.repo.get_download(download_id).await {
                download.output_path = Some(path);
                let _ = self.repo.save_download(download).await;
            }
        }
        let bytes = outcome
            .output_path
            .as_ref()
            .and_then(|p| std::fs::metadata(p).ok())
            .map(|m| m.len());
        self.repo
            .record_history(download_id, outcome.output_path, bytes)
            .await?;
        Ok(())
    }

    /// Apply the terminal state, unless the task was cancelled/changed meanwhile.
    async fn finalize(
        &self,
        task_id: Uuid,
        state: TaskState,
        error: Option<String>,
    ) -> Result<(), ApplicationError> {
        let mut task = self.repo.get_task(task_id).await?;
        if task.state != TaskState::Running {
            // Cancelled or otherwise moved on while the engine ran — leave it.
            return Ok(());
        }
        let error_for_event = match state {
            TaskState::Failed => {
                let msg = error.unwrap_or_default();
                task.fail("engine", msg.clone())?;
                Some(msg)
            }
            other => {
                task.transition_to(other)?;
                None
            }
        };
        self.repo.save_task(task).await?;
        self.publish_state_with_error(task_id, state, error_for_event);
        Ok(())
    }

    fn publish_state(&self, task_id: Uuid, state: TaskState) {
        self.publish_state_with_error(task_id, state, None);
    }

    fn publish_state_with_error(&self, task_id: Uuid, state: TaskState, error: Option<String>) {
        self.events.publish(DomainEvent::TaskStateChanged {
            task_id,
            state,
            error,
        });
    }
}
