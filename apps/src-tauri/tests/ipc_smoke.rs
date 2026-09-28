//! Smoke tests for the IPC command handlers, driven over `AppState` with the
//! Phase 1 default (in-memory repo, stub probe).

use video_dl::ipc::commands;
use video_dl::ipc::dto::{MediaFormatDto, StartDownloadArgs};
use video_dl::ipc::error::IpcErrorKind;
use video_dl::AppState;

#[tokio::test]
async fn start_then_cancel_via_ipc() {
    let app = AppState::default();

    let res = commands::start_download(
        &app,
        StartDownloadArgs {
            url: "https://example.com/video.mp4".into(),
            provider_id: "generic".into(),
            format: MediaFormatDto::ProgressiveMp4,
            title: Some("demo".into()),
            max_height: None,
            audio_only: false,
            referer: None,
        },
    )
    .await
    .unwrap();

    assert!(!res.task_id.is_empty());
    commands::cancel_download(&app, res.task_id).await.unwrap();
}

#[tokio::test]
async fn probe_url_stub_maps_to_probe_error() {
    let app = AppState::default();
    let err = commands::probe_url(&app, "https://example.com/v.mp4".into())
        .await
        .unwrap_err();
    // Default wiring uses the Phase 1 stub probe → Probe error kind.
    assert!(matches!(err.kind, IpcErrorKind::Probe));
}

#[tokio::test]
async fn invalid_url_maps_to_validation_error() {
    let app = AppState::default();
    let err = commands::start_download(
        &app,
        StartDownloadArgs {
            url: "not-a-url".into(),
            provider_id: "generic".into(),
            format: MediaFormatDto::ProgressiveMp4,
            title: None,
            max_height: None,
            audio_only: false,
            referer: None,
        },
    )
    .await
    .unwrap_err();
    assert!(matches!(err.kind, IpcErrorKind::Validation));
}

#[tokio::test]
async fn bad_uuid_maps_to_validation_error() {
    let app = AppState::default();
    let err = commands::cancel_download(&app, "not-a-uuid".into())
        .await
        .unwrap_err();
    assert!(matches!(err.kind, IpcErrorKind::Validation));
}

#[tokio::test]
async fn history_starts_empty() {
    let app = AppState::default();
    let history = commands::list_history(&app).await.unwrap();
    assert!(history.is_empty());
}
