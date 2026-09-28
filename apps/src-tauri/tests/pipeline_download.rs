//! Integration tests for the Phase 1 use-case pipeline, wired through `AppState`
//! with in-memory infrastructure and a fake probe.

use std::sync::Arc;

use async_trait::async_trait;

use video_dl::application::error::ApplicationError;
use video_dl::application::events::DomainEvent;
use video_dl::application::ports::{DownloadRepository, MediaProbe};
use video_dl::application::use_cases::StartDownloadRequest;
use video_dl::domain::entities::{MediaSource, TaskState};
use video_dl::domain::value_objects::{MediaFormat, MediaUrl, Quality};
use video_dl::infrastructure::database::InMemoryDownloadRepository;
use video_dl::infrastructure::events::InMemoryEventBus;
use video_dl::AppState;

/// A probe that always succeeds, returning a single-quality MP4 source.
struct FakeProbe;

#[async_trait]
impl MediaProbe for FakeProbe {
    async fn probe(&self, url: &MediaUrl, _referer: Option<&str>) -> Result<MediaSource, ApplicationError> {
        Ok(MediaSource::new(url.clone(), MediaFormat::ProgressiveMp4)
            .with_qualities(vec![Quality::new(Some(1080), Some(5000), None, true)]))
    }
}

fn app_with(probe: Arc<dyn MediaProbe>) -> (AppState, Arc<dyn DownloadRepository>) {
    let repo: Arc<dyn DownloadRepository> = Arc::new(InMemoryDownloadRepository::new());
    let events = Arc::new(InMemoryEventBus::new(64));
    let app = AppState::new(repo.clone(), probe, events);
    (app, repo)
}

#[tokio::test]
async fn start_creates_a_queued_task() {
    let (app, repo) = app_with(Arc::new(FakeProbe));

    let res = app
        .start_download
        .execute(StartDownloadRequest {
            url: "https://example.com/video.mp4".into(),
            provider_id: "generic".into(),
            format: MediaFormat::ProgressiveMp4,
            title: Some("demo".into()),
            max_height: None,
            audio_only: false,
            referer: None,
        })
        .await
        .unwrap();

    let task = repo.get_task(res.task_id).await.unwrap();
    assert_eq!(task.state, TaskState::Queued);

    let download = repo.get_download(res.download_id).await.unwrap();
    assert_eq!(download.title.as_deref(), Some("demo"));
    assert_eq!(download.provider_id, "generic");
}

#[tokio::test]
async fn cancel_moves_task_to_cancelled() {
    let (app, repo) = app_with(Arc::new(FakeProbe));

    let res = app
        .start_download
        .execute(StartDownloadRequest {
            url: "https://example.com/video.mp4".into(),
            provider_id: "generic".into(),
            format: MediaFormat::ProgressiveMp4,
            title: None,
            max_height: None,
            audio_only: false,
            referer: None,
        })
        .await
        .unwrap();

    app.cancel_download.execute(res.task_id).await.unwrap();

    let task = repo.get_task(res.task_id).await.unwrap();
    assert_eq!(task.state, TaskState::Cancelled);
}

#[tokio::test]
async fn pause_then_resume_requeues() {
    let (app, repo) = app_with(Arc::new(FakeProbe));

    let res = app
        .start_download
        .execute(StartDownloadRequest {
            url: "https://example.com/video.mp4".into(),
            provider_id: "generic".into(),
            format: MediaFormat::ProgressiveMp4,
            title: None,
            max_height: None,
            audio_only: false,
            referer: None,
        })
        .await
        .unwrap();

    // A queued task cannot be paused directly (must be Running).
    assert!(app.pause_resume.pause(res.task_id).await.is_err());

    // Force it Running (the scheduler normally does this), then pause works.
    let mut task = repo.get_task(res.task_id).await.unwrap();
    task.transition_to(TaskState::Running).unwrap();
    repo.save_task(task).await.unwrap();

    app.pause_resume.pause(res.task_id).await.unwrap(); // Running -> Paused
    assert_eq!(
        repo.get_task(res.task_id).await.unwrap().state,
        TaskState::Paused
    );

    // Resume re-queues so the scheduler can continue it.
    app.pause_resume.resume(res.task_id).await.unwrap(); // Paused -> Queued
    assert_eq!(
        repo.get_task(res.task_id).await.unwrap().state,
        TaskState::Queued
    );
}

#[tokio::test]
async fn probe_emits_media_detected_event() {
    let (app, _repo) = app_with(Arc::new(FakeProbe));
    let mut rx = app.event_bus.subscribe();

    let source = app
        .probe_url
        .execute("https://example.com/video.mp4")
        .await
        .unwrap();
    assert_eq!(source.format, MediaFormat::ProgressiveMp4);

    let evt = rx.try_recv().unwrap();
    assert_eq!(evt, DomainEvent::MediaDetected { source });
}

#[tokio::test]
async fn probe_rejects_invalid_url() {
    let (app, _repo) = app_with(Arc::new(FakeProbe));
    assert!(app.probe_url.execute("not-a-url").await.is_err());
}

#[tokio::test]
async fn remove_download_deletes_a_failed_task_for_good() {
    let (app, repo) = app_with(Arc::new(FakeProbe));

    let res = app
        .start_download
        .execute(StartDownloadRequest {
            url: "https://example.com/video.mp4".into(),
            provider_id: "generic".into(),
            format: MediaFormat::ProgressiveMp4,
            title: None,
            max_height: None,
            audio_only: false,
            referer: None,
        })
        .await
        .unwrap();

    let mut task = repo.get_task(res.task_id).await.unwrap();
    task.transition_to(TaskState::Running).unwrap();
    task.fail("engine", "boom").unwrap();
    repo.save_task(task).await.unwrap();

    app.remove_download.execute(res.task_id).await.unwrap();

    // Gone for good — not just hidden from a view.
    assert!(repo.get_task(res.task_id).await.is_err());
    assert!(repo.get_download(res.download_id).await.is_err());
}

#[tokio::test]
async fn retry_download_requeues_a_failed_task() {
    let (app, repo) = app_with(Arc::new(FakeProbe));

    let res = app
        .start_download
        .execute(StartDownloadRequest {
            url: "https://example.com/video.mp4".into(),
            provider_id: "generic".into(),
            format: MediaFormat::ProgressiveMp4,
            title: None,
            max_height: None,
            audio_only: false,
            referer: None,
        })
        .await
        .unwrap();

    let mut task = repo.get_task(res.task_id).await.unwrap();
    task.transition_to(TaskState::Running).unwrap();
    task.fail("engine", "403 Forbidden").unwrap();
    repo.save_task(task).await.unwrap();

    app.retry_download.execute(res.task_id).await.unwrap();

    let task = repo.get_task(res.task_id).await.unwrap();
    assert_eq!(task.state, TaskState::Queued);
    assert_eq!(task.error_message, None);
}

#[tokio::test]
async fn remove_download_refuses_a_still_running_task() {
    let (app, repo) = app_with(Arc::new(FakeProbe));

    let res = app
        .start_download
        .execute(StartDownloadRequest {
            url: "https://example.com/video.mp4".into(),
            provider_id: "generic".into(),
            format: MediaFormat::ProgressiveMp4,
            title: None,
            max_height: None,
            audio_only: false,
            referer: None,
        })
        .await
        .unwrap();

    // Still Queued — not a terminal state.
    let err = app.remove_download.execute(res.task_id).await.unwrap_err();
    assert!(err.to_string().contains("échoué ou annulé"));
    assert!(repo.get_task(res.task_id).await.is_ok());
}

#[tokio::test]
async fn delete_history_entry_removes_it_from_the_list() {
    let (app, repo) = app_with(Arc::new(FakeProbe));

    let res = app
        .start_download
        .execute(StartDownloadRequest {
            url: "https://example.com/video.mp4".into(),
            provider_id: "generic".into(),
            format: MediaFormat::ProgressiveMp4,
            title: None,
            max_height: None,
            audio_only: false,
            referer: None,
        })
        .await
        .unwrap();
    repo.record_history(res.download_id, Some("C:/out.mp4".into()), Some(123))
        .await
        .unwrap();
    assert_eq!(app.list_history.execute(10).await.unwrap().len(), 1);

    app.delete_history_entry.execute(res.download_id).await.unwrap();

    assert_eq!(app.list_history.execute(10).await.unwrap().len(), 0);
}

#[tokio::test]
async fn default_app_uses_stub_probe() {
    // Default wiring uses the Phase 1 stub probe, which errors by design.
    let app = AppState::default();
    assert!(app
        .probe_url
        .execute("https://example.com/video.mp4")
        .await
        .is_err());
}
