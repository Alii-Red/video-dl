use std::sync::Arc;

use crate::application::error::ApplicationError;
use crate::application::ports::{
    DownloadRepository, EventPublisher, MediaProbe, SettingsRepository, TaskController,
};
use crate::application::use_cases::{
    CancelDownload, DeleteHistoryEntry, GetSettings, ListActiveDownloads, ListHistory,
    PauseResume, ProbeUrl, RecoverInterruptedTasks, RemoveDownload, RetryDownload, StartDownload,
    UpdateSettings,
};
use crate::infrastructure::database::{
    InMemoryDownloadRepository, InMemorySettingsRepository, SqlxDownloadRepository,
    SqlxSettingsRepository,
};
use crate::infrastructure::events::InMemoryEventBus;
use crate::infrastructure::network::StubMediaProbe;
use crate::infrastructure::scheduler::RunningTasks;

/// Composition root. Wires infrastructure implementations into the use cases.
///
/// Phase 1 defaults to in-memory infra. Later phases swap in SQLx, the yt-dlp
/// engine and the real probe by changing only this wiring — use cases are
/// untouched because they depend on ports, not implementations.
pub struct AppState {
    pub probe_url: ProbeUrl,
    pub start_download: StartDownload,
    pub cancel_download: CancelDownload,
    pub pause_resume: PauseResume,
    pub list_history: ListHistory,
    pub list_active: ListActiveDownloads,
    pub recover: RecoverInterruptedTasks,
    pub get_settings: GetSettings,
    pub update_settings: UpdateSettings,
    pub remove_download: RemoveDownload,
    pub retry_download: RetryDownload,
    pub delete_history_entry: DeleteHistoryEntry,
    pub event_bus: Arc<InMemoryEventBus>,
    /// Shared registry of running tasks. Hand `running.clone()` to the
    /// `TaskScheduler` so cancel/pause reach the same in-flight processes.
    pub running: RunningTasks,
    /// Exposed so the desktop shell can build a `TaskScheduler` sharing the same
    /// storage as the use cases above.
    pub download_repo: Arc<dyn DownloadRepository>,
    pub event_publisher: Arc<dyn EventPublisher>,
}

impl AppState {
    /// Full wiring from explicit dependencies.
    pub fn assemble(
        download_repo: Arc<dyn DownloadRepository>,
        settings_repo: Arc<dyn SettingsRepository>,
        probe: Arc<dyn MediaProbe>,
        events: Arc<InMemoryEventBus>,
    ) -> Self {
        let publisher: Arc<dyn EventPublisher> = events.clone();
        let running = RunningTasks::new();
        let controller: Arc<dyn TaskController> = Arc::new(running.clone());
        Self {
            probe_url: ProbeUrl::new(probe, publisher.clone()),
            start_download: StartDownload::new(download_repo.clone(), publisher.clone()),
            cancel_download: CancelDownload::new(
                download_repo.clone(),
                publisher.clone(),
                controller.clone(),
            ),
            pause_resume: PauseResume::new(download_repo.clone(), publisher.clone(), controller),
            list_history: ListHistory::new(download_repo.clone()),
            list_active: ListActiveDownloads::new(download_repo.clone()),
            recover: RecoverInterruptedTasks::new(download_repo.clone(), publisher.clone()),
            get_settings: GetSettings::new(settings_repo.clone()),
            update_settings: UpdateSettings::new(settings_repo),
            remove_download: RemoveDownload::new(download_repo.clone()),
            retry_download: RetryDownload::new(download_repo.clone(), publisher.clone()),
            delete_history_entry: DeleteHistoryEntry::new(download_repo.clone()),
            event_bus: events,
            running,
            download_repo,
            event_publisher: publisher,
        }
    }

    /// Convenience wiring with an in-memory settings store. Tests use this to
    /// inject a fake download repo/probe.
    pub fn new(
        repo: Arc<dyn DownloadRepository>,
        probe: Arc<dyn MediaProbe>,
        events: Arc<InMemoryEventBus>,
    ) -> Self {
        let settings: Arc<dyn SettingsRepository> = Arc::new(InMemorySettingsRepository::new());
        Self::assemble(repo, settings, probe, events)
    }

    /// Production wiring: open the SQLite database (creating + migrating it),
    /// share its pool with the settings store, run crash recovery, and return the
    /// ready app. `probe` is injected because the real detection stack is chosen
    /// by the caller (pass `HttpMediaProbe` in production, `StubMediaProbe` else).
    pub async fn bootstrap_sqlite(
        db_path: &str,
        probe: Arc<dyn MediaProbe>,
    ) -> Result<Self, ApplicationError> {
        let download = Arc::new(SqlxDownloadRepository::connect_file(db_path, 5).await?);
        let settings: Arc<dyn SettingsRepository> =
            Arc::new(SqlxSettingsRepository::from_pool(download.pool()));
        let events = Arc::new(InMemoryEventBus::new(256));
        let app = Self::assemble(download, settings, probe, events);
        app.recover.execute().await?;
        Ok(app)
    }
}

impl Default for AppState {
    /// Phase 1 default wiring: everything in-memory, stub probe.
    fn default() -> Self {
        let repo: Arc<dyn DownloadRepository> = Arc::new(InMemoryDownloadRepository::new());
        let probe: Arc<dyn MediaProbe> = Arc::new(StubMediaProbe);
        let events = Arc::new(InMemoryEventBus::new(256));
        Self::new(repo, probe, events)
    }
}
