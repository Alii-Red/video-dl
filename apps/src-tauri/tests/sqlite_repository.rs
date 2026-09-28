//! Integration tests for the SQLx repository against an in-memory SQLite DB.

use std::sync::Arc;

use video_dl::application::ports::{DownloadRepository, EventPublisher};
use video_dl::application::use_cases::RecoverInterruptedTasks;
use video_dl::domain::entities::{Download, Task, TaskState};
use video_dl::domain::value_objects::{MediaFormat, MediaUrl};
use video_dl::infrastructure::database::SqlxDownloadRepository;
use video_dl::infrastructure::events::InMemoryEventBus;

async fn repo() -> SqlxDownloadRepository {
    SqlxDownloadRepository::in_memory()
        .await
        .expect("in-memory db")
}

fn sample() -> (Download, Task) {
    let url = MediaUrl::parse("https://example.com/v.mp4").unwrap();
    let mut download = Download::new(url, "generic", MediaFormat::ProgressiveMp4);
    download.title = Some("clip".into());
    let task = Task::new(download.id);
    (download, task)
}

#[tokio::test]
async fn create_and_read_back() {
    let repo = repo().await;
    let (download, task) = sample();
    let (did, tid) = (download.id, task.id);
    repo.create(download.clone(), task.clone()).await.unwrap();

    let got_d = repo.get_download(did).await.unwrap();
    assert_eq!(got_d, download);

    let got_t = repo.get_task(tid).await.unwrap();
    assert_eq!(got_t.state, TaskState::Queued);
    assert_eq!(got_t.download_id, did);
}

#[tokio::test]
async fn save_task_updates_state_and_lists_by_state() {
    let repo = repo().await;
    let (download, mut task) = sample();
    repo.create(download, task.clone()).await.unwrap();

    task.transition_to(TaskState::Running).unwrap();
    repo.save_task(task.clone()).await.unwrap();

    let running = repo.list_tasks_by_state(TaskState::Running).await.unwrap();
    assert_eq!(running.len(), 1);
    assert_eq!(running[0].id, task.id);

    let queued = repo.list_tasks_by_state(TaskState::Queued).await.unwrap();
    assert!(queued.is_empty());
}

#[tokio::test]
async fn missing_ids_report_not_found() {
    let repo = repo().await;
    let random = uuid_v4();
    assert!(repo.get_task(random).await.is_err());
    assert!(repo.get_download(random).await.is_err());
}

#[tokio::test]
async fn crash_recovery_requeues_running_tasks() {
    let repo = Arc::new(repo().await);
    let (download, mut task) = sample();
    repo.create(download, task.clone()).await.unwrap();

    // Simulate a task that was Running when the app crashed.
    task.transition_to(TaskState::Running).unwrap();
    repo.save_task(task.clone()).await.unwrap();

    let events = Arc::new(InMemoryEventBus::new(16));
    let publisher: Arc<dyn EventPublisher> = events.clone();
    let recovered = RecoverInterruptedTasks::new(repo.clone(), publisher)
        .execute()
        .await
        .unwrap();
    assert_eq!(recovered, 1);

    let got = repo.get_task(task.id).await.unwrap();
    assert_eq!(got.state, TaskState::Queued);
}

#[tokio::test]
async fn list_active_excludes_terminal_tasks() {
    let repo = repo().await;

    // One that will complete, one that stays queued.
    let (d1, mut t1) = sample();
    repo.create(d1, t1.clone()).await.unwrap();
    t1.transition_to(TaskState::Running).unwrap();
    t1.transition_to(TaskState::Completed).unwrap();
    repo.save_task(t1).await.unwrap();

    let (d2, t2) = sample();
    let active_id = t2.id;
    repo.create(d2, t2).await.unwrap();

    let active = repo.list_active().await.unwrap();
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].1.id, active_id);
    assert_eq!(active[0].1.state, TaskState::Queued);
}

#[tokio::test]
async fn history_records_and_reads_with_join() {
    let repo = repo().await;
    let (download, task) = sample();
    let download_id = download.id;
    let source_url = download.source_url.as_str().to_string();
    repo.create(download, task).await.unwrap();

    repo.record_history(download_id, Some("C:/out/clip.mp4".into()), Some(12345))
        .await
        .unwrap();

    let history = repo.recent_history(10).await.unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].download_id, download_id);
    assert_eq!(history[0].title.as_deref(), Some("clip")); // joined from downloads
    assert_eq!(history[0].source_url, source_url);
    assert_eq!(history[0].bytes, Some(12345));
    assert_eq!(history[0].final_path.as_deref(), Some("C:/out/clip.mp4"));
}

fn uuid_v4() -> uuid::Uuid {
    // Re-parse a fixed-shape random string via Download to avoid a direct uuid dep here.
    Download::new(
        MediaUrl::parse("https://x.com/a").unwrap(),
        "generic",
        MediaFormat::Hls,
    )
    .id
}
