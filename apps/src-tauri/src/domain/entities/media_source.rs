use serde::{Deserialize, Serialize};

use crate::domain::value_objects::{MediaFormat, MediaUrl, Quality};

/// A detected media, with its available qualities. Produced by the probe/parsers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MediaSource {
    pub url: MediaUrl,
    pub format: MediaFormat,
    pub title: Option<String>,
    pub qualities: Vec<Quality>,
    /// True when the stream is protected (encrypted HLS key / DASH ContentProtection).
    /// Protected sources are surfaced to the UI but never auto-downloaded.
    pub protected: bool,
    /// Poster/preview image URL, captured from the source page at detection
    /// time (`<video poster>` or `og:image`) — best-effort, never fetched or
    /// generated server-side.
    pub thumbnail: Option<String>,
    /// Video duration in whole seconds, read from the page's `<video>`
    /// element when its metadata happened to be loaded at detection time.
    pub duration_secs: Option<u32>,
    /// Origin browser tab, when detected live via the Opera GX extension.
    /// `None` for every other detection source (manual probe, page
    /// extraction) — those aren't tied to a live browsing context, so the
    /// widget never filters them out.
    pub source_tab_id: Option<u32>,
    /// The page URL open in `source_tab_id` at detection time. Used together
    /// with `source_tab_id` to scope which detections the widget shows for
    /// the currently active tab/page.
    pub page_url: Option<String>,
}

impl MediaSource {
    pub fn new(url: MediaUrl, format: MediaFormat) -> Self {
        Self {
            url,
            format,
            title: None,
            qualities: Vec::new(),
            protected: false,
            thumbnail: None,
            duration_secs: None,
            source_tab_id: None,
            page_url: None,
        }
    }

    pub fn with_qualities(mut self, qualities: Vec<Quality>) -> Self {
        self.qualities = qualities;
        self
    }

    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn with_thumbnail(mut self, thumbnail: impl Into<String>) -> Self {
        self.thumbnail = Some(thumbnail.into());
        self
    }

    pub fn with_duration_secs(mut self, duration_secs: u32) -> Self {
        self.duration_secs = Some(duration_secs);
        self
    }

    pub fn with_browsing_context(mut self, tab_id: u32, page_url: Option<String>) -> Self {
        self.source_tab_id = Some(tab_id);
        self.page_url = page_url;
        self
    }
}
