//! Settings persistence tests: in-memory + SQLite, plus the IPC round-trip.

use video_dl::application::ports::SettingsRepository;
use video_dl::domain::value_objects::Settings;
use video_dl::infrastructure::database::{
    InMemorySettingsRepository, SqlxDownloadRepository, SqlxSettingsRepository,
};
use video_dl::ipc::commands;
use video_dl::ipc::dto::SettingsDto;
use video_dl::AppState;

#[tokio::test]
async fn in_memory_defaults_then_saves() {
    let repo = InMemorySettingsRepository::new();
    assert_eq!(repo.load().await.unwrap(), Settings::default());

    let custom = Settings {
        download_dir: "C:/dl".into(),
        concurrency: 4,
        max_height: Some(1080),
        audio_only: false,
        subtitles: true,
        rate_limit_bps: Some(1_000_000),
        ..Settings::default()
    };
    repo.save(&custom).await.unwrap();
    assert_eq!(repo.load().await.unwrap(), custom);
}

#[tokio::test]
async fn sqlite_persists_settings_across_repositories() {
    // Two settings repos sharing one SQLite file must see the same data.
    let download = SqlxDownloadRepository::in_memory().await.unwrap();
    let settings = SqlxSettingsRepository::from_pool(download.pool());

    // Defaults when nothing saved yet.
    assert_eq!(settings.load().await.unwrap(), Settings::default());

    let custom = Settings {
        download_dir: "D:/videos".into(),
        concurrency: 3,
        max_height: Some(720),
        audio_only: true,
        subtitles: false,
        rate_limit_bps: None,
        ..Settings::default()
    };
    settings.save(&custom).await.unwrap();

    let other = SqlxSettingsRepository::from_pool(download.pool());
    assert_eq!(other.load().await.unwrap(), custom);
}

#[tokio::test]
async fn ipc_get_and_update_settings() {
    let app = AppState::default();

    let initial = commands::get_settings(&app).await.unwrap();
    assert_eq!(initial.concurrency, 2);

    let updated = SettingsDto {
        download_dir: "C:/out".into(),
        concurrency: 5,
        max_height: Some(2160),
        audio_only: false,
        subtitles: true,
        rate_limit_bps: Some(2_000_000),
        window_mode: video_dl::ipc::dto::WindowModeDto::Volatile,
        volatile_geometry: None,
        extension_enabled: true,
        extension_port: 8787,
        extension_token: "tok".into(),
    };
    commands::update_settings(&app, updated.clone()).await.unwrap();

    let read_back = commands::get_settings(&app).await.unwrap();
    assert_eq!(read_back, updated);
}
