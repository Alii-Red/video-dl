use async_trait::async_trait;

use crate::application::error::ApplicationError;

/// One entry of a resolved playlist: a single, individually downloadable video.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaylistEntry {
    pub url: String,
    pub title: Option<String>,
    pub duration_secs: Option<u32>,
    pub thumbnail: Option<String>,
}

/// A playlist/channel/showcase URL expanded into its entries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedPlaylist {
    pub title: String,
    pub entries: Vec<PlaylistEntry>,
}

/// Expands a playlist URL into its individual videos without downloading
/// anything. Each entry is then queued as an ordinary single-video download,
/// so the engine/scheduler/progress pipeline is reused unchanged.
#[async_trait]
pub trait PlaylistResolver: Send + Sync {
    /// `Ok(None)` when the URL is not a playlist (the caller falls back to the
    /// normal single-media path).
    async fn resolve(&self, url: &str) -> Result<Option<ResolvedPlaylist>, ApplicationError>;
}
