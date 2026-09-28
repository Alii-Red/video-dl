use async_trait::async_trait;

use crate::application::error::ApplicationError;
use crate::application::ports::EngineRequest;
use crate::domain::entities::Download;

/// Turns a [`Download`] into a concrete [`EngineRequest`] (URL, output dir,
/// yt-dlp args). Implemented by the provider registry in Phase 7 — the scheduler
/// depends on this port, not on any provider.
#[async_trait]
pub trait DownloadPlanner: Send + Sync {
    async fn plan(&self, download: &Download) -> Result<EngineRequest, ApplicationError>;
}
