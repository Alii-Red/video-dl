use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::application::error::ApplicationError;

/// A candidate media URL observed while extracting a page, paired with the
/// page's title at the time of observation (if any) — used as a human
/// readable download filename instead of the raw manifest URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractedCandidate {
    pub url: String,
    pub title: Option<String>,
    /// Poster/preview image URL captured from the page (`<video poster>` or
    /// `og:image`), if any.
    pub thumbnail: Option<String>,
    /// Video duration in whole seconds, if the page's `<video>` element had
    /// already loaded its metadata at observation time.
    pub duration_secs: Option<u32>,
}

/// Extracts candidate media/manifest URLs from a web page by loading it in a
/// real browser engine and observing the network requests it makes (fetch/XHR
/// hooked via an injected script) — the "manual import" pain point solver from
/// ADR Partie 4/6, for pages that only reveal their media URL through JS.
///
/// Kept Tauri-free: the concrete implementation (which needs to create actual
/// windows) lives in the desktop shell. This port only deals in plain data and
/// a tokio channel.
#[async_trait]
pub trait PageMediaExtractor: Send + Sync {
    /// Load `url` in a hidden window, collect every candidate observed within
    /// `timeout`, then close the window and return what was found.
    async fn extract_silent(
        &self,
        url: &str,
        timeout: Duration,
    ) -> Result<Vec<ExtractedCandidate>, ApplicationError>;

    /// Load `url` in a visible window so the user can interact with the page
    /// (e.g. click play). Returns a receiver that yields each candidate as
    /// it's observed; the channel closes when the user closes the window.
    async fn extract_interactive(
        &self,
        url: &str,
    ) -> Result<mpsc::UnboundedReceiver<ExtractedCandidate>, ApplicationError>;
}
