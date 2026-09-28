use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::application::error::ApplicationError;
use crate::domain::value_objects::Progress;

/// What the engine needs to run a download. Args are built by the provider.
#[derive(Debug, Clone)]
pub struct EngineRequest {
    pub url: String,
    pub output_dir: String,
    pub ytdlp_args: Vec<String>,
    /// Human-readable title, when known (e.g. the source page's title for an
    /// extracted HLS/DASH manifest). Used to name the output file instead of
    /// falling back to yt-dlp's default, which for a raw manifest URL is
    /// often the manifest's own filename — including its signed query
    /// string, since HLS hosts commonly embed an auth token there.
    pub title: Option<String>,
    /// When set, this is a plain HTTP file to stream directly instead of
    /// handing `url` to yt-dlp. Used for sites (tube/KVS players) whose
    /// direct file link yt-dlp's generic extractor refuses — the link is a
    /// real, already-probed `video/mp4` etc, so no extraction is needed, just
    /// a GET with the right `Referer`.
    pub direct_url: Option<String>,
    /// `Referer`/`Origin` header for `direct_url` (the page it came from) —
    /// many hosts reject a hotlinked GET without it.
    pub referer: Option<String>,
}

/// Result of a finished download run.
#[derive(Debug, Clone, PartialEq)]
pub struct EngineOutcome {
    pub output_path: Option<String>,
}

/// Execution port. Phase 1 defines the contract; the yt-dlp implementation
/// (spawn + stdout parsing) arrives in Phase 4.
#[async_trait]
pub trait DownloadEngine: Send + Sync {
    /// Run a download to completion, reporting progress on `progress`.
    async fn run(
        &self,
        request: EngineRequest,
        progress: mpsc::Sender<Progress>,
    ) -> Result<EngineOutcome, ApplicationError>;
}
