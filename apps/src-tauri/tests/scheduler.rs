//! Scheduler integration tests with a fake engine + planner and in-memory repo.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::mpsc;
use uuid::Uuid;

use video_dl::application::error::ApplicationError;
use video_dl::application::ports::{
    DownloadEngine, DownloadPlanner, DownloadRepository, EngineOutcome, EngineRequest,
    EventPublisher, TaskController,
};
use video_dl::application::use_cases::CancelDownload;
use video_dl::domain::entities::{Download, Task, TaskState};
use video_dl::domain::value_objects::{MediaFormat, MediaUrl, Progress};
use video_dl::infrastructure::database::InMemoryDownloadRepository;
use video_dl::infrastructure::events::InMemoryEventBus;
use video_dl::infrastructure::scheduler::{RunningTasks, TaskScheduler};

struct OkEngine;
#[async_trait]
impl DownloadEngine for OkEngine {
    async fn run(
        &self,
        _req: EngineRequest,
        progress: mpsc::Sender<Progress>,
    ) -> Result<EngineOutcome, ApplicationError> {
        let _ = progress.send(Progress::clamped(50.0)).await;
        let _ = progress.send(Progress::clamped(100.0)).await;
        Ok(EngineOutcome {
            output_path: Some("out.mp4".into()),
        })
    }
}

struct FailEngine;
#[async_trait]
impl DownloadEngine for FailEngine {
    async fn run(
        &self,
        _req: EngineRequest,
        _progress: mpsc::Sender<Progress>,
    ) -> Result<EngineOutcome, ApplicationError> {
        Err(ApplicationError::Engine("boom".into()))
    }
}

/// Never returns on its own — used to test interruption.
struct SlowEngine;
#[async_trait]
impl DownloadEngine for SlowEngine {
    async fn run(
        &self,
        _req: EngineRequest,
        progress: mpsc::Sender<Progress>,
    ) -> Result<EngineOutcome, ApplicationError> {
        loop {
            let _ = progress.send(Progress::clamped(1.0)).await;
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }
}

/// Completes immediately, reporting a given output path.
struct PathEngine(String);
#[async_trait]
impl DownloadEngine for PathEngine {
    async fn run(
        &self,
        _req: EngineRequest,
        progress: mpsc::Sender<Progress>,
    ) -> Result<EngineOutcome, ApplicationError> {
        let _ = progress.send(Progress::clamped(100.0)).await;
        Ok(EngineOutcome {
            output_path: Some(self.0.clone()),
        })
    }
}

struct DummyPlanner;
#[async_trait]
impl DownloadPlanner for DummyPlanner {
    async fn plan(&self, download: &Download) -> Result<EngineRequest, ApplicationError> {
        Ok(EngineRequest {
            url: download.source_url.as_str().to_string(),
            output_dir: ".".into(),
            ytdlp_args: vec![],
            title: download.title.clone(),
            direct_url: None,
            referer: None,
        })
    }
}

async fn setup(
    engine: Arc<dyn DownloadEngine>,
) -> (TaskScheduler, Arc<dyn DownloadRepository>, Uuid) {
    let repo: Arc<dyn DownloadRepository> = Arc::new(InMemoryDownloadRepository::new());
    let events: Arc<dyn EventPublisher> = Arc::new(InMemoryEventBus::new(64));
    let planner: Arc<dyn DownloadPlanner> = Arc::new(DummyPlanner);
    let scheduler = TaskScheduler::new(repo.clone(), engine, planner, events, 2, RunningTasks::new());

    let url = MediaUrl::parse("https://example.com/v.mp4").unwrap();
    let download = Download::new(url, "generic", MediaFormat::ProgressiveMp4);
    let task = Task::new(download.id);
    let task_id = task.id;
    repo.create(download, task).await.unwrap();

    (scheduler, repo, task_id)
}

#[tokio::test]
async fn run_pending_completes_task_and_records_progress() {
    let (scheduler, repo, task_id) = setup(Arc::new(OkEngine)).await;

    scheduler.run_pending().await.unwrap();

    let task = repo.get_task(task_id).await.unwrap();
    assert_eq!(task.state, TaskState::Completed);
    assert_eq!(task.progress.percent, 100.0);
}

#[tokio::test]
async fn failing_engine_marks_task_failed() {
    let (scheduler, repo, task_id) = setup(Arc::new(FailEngine)).await;

    // run_task surfaces the error; the task is still marked Failed.
    let result = scheduler.run_task(task_id).await;
    assert!(result.is_err());

    let task = repo.get_task(task_id).await.unwrap();
    assert_eq!(task.state, TaskState::Failed);
    assert_eq!(task.error_code.as_deref(), Some("engine"));
}

#[tokio::test]
async fn run_pending_with_no_queued_tasks_is_ok() {
    let repo: Arc<dyn DownloadRepository> = Arc::new(InMemoryDownloadRepository::new());
    let events: Arc<dyn EventPublisher> = Arc::new(InMemoryEventBus::new(8));
    let planner: Arc<dyn DownloadPlanner> = Arc::new(DummyPlanner);
    let scheduler =
        TaskScheduler::new(repo, Arc::new(OkEngine), planner, events, 1, RunningTasks::new());
    scheduler.run_pending().await.unwrap();
}

#[tokio::test]
async fn completion_persists_output_path_and_history() {
    let file = std::env::temp_dir().join(format!("vdl-sch-{}.bin", Uuid::new_v4()));
    std::fs::write(&file, b"hello world").unwrap();
    let path = file.to_string_lossy().to_string();

    let repo: Arc<dyn DownloadRepository> = Arc::new(InMemoryDownloadRepository::new());
    let events: Arc<dyn EventPublisher> = Arc::new(InMemoryEventBus::new(16));
    let planner: Arc<dyn DownloadPlanner> = Arc::new(DummyPlanner);
    let scheduler = TaskScheduler::new(
        repo.clone(),
        Arc::new(PathEngine(path.clone())),
        planner,
        events,
        1,
        RunningTasks::new(),
    );

    let url = MediaUrl::parse("https://example.com/v.mp4").unwrap();
    let download = Download::new(url, "generic", MediaFormat::ProgressiveMp4);
    let download_id = download.id;
    let task = Task::new(download.id);
    let task_id = task.id;
    repo.create(download, task).await.unwrap();

    scheduler.run_task(task_id).await.unwrap();

    // output_path persisted on the download.
    let dl = repo.get_download(download_id).await.unwrap();
    assert_eq!(dl.output_path.as_deref(), Some(path.as_str()));

    // history archived with the file size.
    let history = repo.recent_history(10).await.unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].download_id, download_id);
    assert_eq!(history[0].bytes, Some(11)); // "hello world"
    assert_eq!(history[0].final_path.as_deref(), Some(path.as_str()));

    std::fs::remove_file(&file).ok();
}

#[tokio::test]
async fn cancel_interrupts_a_running_task() {
    let repo: Arc<dyn DownloadRepository> = Arc::new(InMemoryDownloadRepository::new());
    let events = Arc::new(InMemoryEventBus::new(64));
    let publisher: Arc<dyn EventPublisher> = events.clone();
    let planner: Arc<dyn DownloadPlanner> = Arc::new(DummyPlanner);
    let running = RunningTasks::new();
    let controller: Arc<dyn TaskController> = Arc::new(running.clone());
    let scheduler = TaskScheduler::new(
        repo.clone(),
        Arc::new(SlowEngine),
        planner,
        publisher.clone(),
        1,
        running.clone(),
    );

    let url = MediaUrl::parse("https://example.com/v.mp4").unwrap();
    let download = Download::new(url, "generic", MediaFormat::ProgressiveMp4);
    let task = Task::new(download.id);
    let task_id = task.id;
    repo.create(download, task).await.unwrap();

    // Start the (never-ending) task in the background.
    let sched = scheduler.clone();
    let handle = tokio::spawn(async move { sched.run_task(task_id).await });

    // Wait until it is actually registered as running.
    for _ in 0..200 {
        if running.is_running(task_id) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert!(running.is_running(task_id), "task never started");

    // Cancel via the use case: sets Cancelled *and* interrupts the process.
    let cancel = CancelDownload::new(repo.clone(), publisher, controller);
    cancel.execute(task_id).await.unwrap();

    // run_task must return promptly now that the engine future was dropped.
    let joined = tokio::time::timeout(Duration::from_secs(3), handle).await;
    assert!(joined.is_ok(), "run_task did not stop after cancel");

    let final_task = repo.get_task(task_id).await.unwrap();
    assert_eq!(final_task.state, TaskState::Cancelled);
    assert!(!running.is_running(task_id), "token should be cleaned up");
}
