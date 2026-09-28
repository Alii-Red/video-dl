use std::sync::Arc;

use uuid::Uuid;

use crate::application::error::ApplicationError;
use crate::application::events::DomainEvent;
use crate::application::ports::{DownloadRepository, EventPublisher};
use crate::domain::entities::{Download, Task};
use crate::domain::value_objects::{MediaFormat, MediaUrl};

pub struct StartDownloadRequest {
    pub url: String,
    pub provider_id: String,
    pub format: MediaFormat,
    pub title: Option<String>,
    pub max_height: Option<u32>,
    pub audio_only: bool,
    pub referer: Option<String>,
}

pub struct StartDownloadResult {
    pub download_id: Uuid,
    pub task_id: Uuid,
}

/// Create a download + its queued task, persist them, and announce creation.
/// The scheduler (Phase 5) picks the queued task up and runs the engine.
pub struct StartDownload {
    repo: Arc<dyn DownloadRepository>,
    events: Arc<dyn EventPublisher>,
}

impl StartDownload {
    pub fn new(repo: Arc<dyn DownloadRepository>, events: Arc<dyn EventPublisher>) -> Self {
        Self { repo, events }
    }

    pub async fn execute(
        &self,
        req: StartDownloadRequest,
    ) -> Result<StartDownloadResult, ApplicationError> {
        let url = MediaUrl::parse(&req.url)?;
        let mut download = Download::new(url, req.provider_id, req.format);
        download.title = req.title;
        download.max_height = req.max_height;
        download.audio_only = req.audio_only;
        download.referer = req.referer;

        let task = Task::new(download.id);
        let download_id = download.id;
        let task_id = task.id;

        self.repo.create(download, task).await?;
        self.events.publish(DomainEvent::DownloadCreated {
            download_id,
            task_id,
        });

        Ok(StartDownloadResult {
            download_id,
            task_id,
        })
    }
}
