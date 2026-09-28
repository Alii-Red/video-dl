use std::sync::Arc;

use crate::application::error::ApplicationError;
use crate::application::events::DomainEvent;
use crate::application::ports::{EventPublisher, MediaProbe};
use crate::domain::entities::MediaSource;
use crate::domain::value_objects::{MediaFormat, MediaUrl};

/// Page-context metadata that rides along with a detected URL, captured
/// best-effort at the point of observation (injected hook / extension
/// content script) — never fetched or generated server-side.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DetectionMeta {
    pub title: Option<String>,
    pub thumbnail: Option<String>,
    pub duration_secs: Option<u32>,
    /// Origin browser tab + page, when known (extension source only) — lets
    /// the widget scope its display to the currently active tab/page.
    pub source_tab_id: Option<u32>,
    pub page_url: Option<String>,
}

/// Ingests a single candidate media URL (from any source — page extraction or
/// the browser-extension WS bridge): validate, probe through the standard
/// pipeline, attach page metadata if provided, and publish `MediaDetected`.
///
/// Shared so every detection source converges on identical HLS/DASH parsing,
/// protection detection, and event shape.
pub struct IngestDetection {
    probe: Arc<dyn MediaProbe>,
    events: Arc<dyn EventPublisher>,
}

impl IngestDetection {
    pub fn new(probe: Arc<dyn MediaProbe>, events: Arc<dyn EventPublisher>) -> Self {
        Self { probe, events }
    }

    /// Returns the probed source on success. A failed probe (noise that slipped
    /// past the caller's prefilter) yields `None` rather than an error — callers
    /// process a stream of candidates and shouldn't abort on one bad URL.
    pub async fn execute(&self, raw_url: &str, meta: DetectionMeta) -> Option<MediaSource> {
        let url = MediaUrl::parse(raw_url).ok()?;
        let mut source = self
            .probe
            .probe(&url, meta.page_url.as_deref())
            .await
            .ok()?;
        source = apply_meta(source, meta);
        self.events.publish(DomainEvent::MediaDetected {
            source: source.clone(),
        });
        Some(source)
    }

    /// Same as [`Self::execute`] but surfaces validation/probe errors — useful
    /// when the caller wants to distinguish "not a media URL" from success.
    pub async fn try_execute(
        &self,
        raw_url: &str,
        meta: DetectionMeta,
    ) -> Result<MediaSource, ApplicationError> {
        let url = MediaUrl::parse(raw_url)?;
        let mut source = self.probe.probe(&url, meta.page_url.as_deref()).await?;
        source = apply_meta(source, meta);
        self.events.publish(DomainEvent::MediaDetected {
            source: source.clone(),
        });
        Ok(source)
    }

    /// Publishes a `BrowsingContextChanged` — the extension's active
    /// tab/page changed (tab switch or navigation). Doesn't touch any
    /// already-detected sources; consumers (the widget) use it to scope
    /// which detections they *show*, not to discard history.
    pub fn publish_context_change(&self, tab_id: u32, page_url: Option<String>) {
        self.events
            .publish(DomainEvent::BrowsingContextChanged { tab_id, page_url });
    }

    /// Ingests a *page* the extension recognized as yt-dlp-native (YouTube,
    /// ...). Unlike [`Self::execute`], the URL is NOT probed — it's an HTML
    /// page, not a media file; probing would just fail. yt-dlp resolves the
    /// actual streams itself at download time (the planner re-routes by host
    /// to the matching provider). The page URL doubles as the browsing
    /// context so the widget scopes it to the tab it came from.
    pub fn ingest_page(
        &self,
        page_url: &str,
        tab_id: u32,
        title: Option<String>,
    ) -> Option<MediaSource> {
        let url = MediaUrl::parse(page_url).ok()?;
        // Format is informational here (the youtube/generic provider builds
        // its own yt-dlp args); mp4 reflects the usual muxed output.
        let mut source = MediaSource::new(url, MediaFormat::ProgressiveMp4)
            .with_browsing_context(tab_id, Some(page_url.to_string()));
        if let Some(title) = title.filter(|t| !t.trim().is_empty()) {
            source = source.with_title(title);
        }
        self.events.publish(DomainEvent::MediaDetected {
            source: source.clone(),
        });
        Some(source)
    }

    /// Ingests a *direct file* link the extension pulled straight from a
    /// page's HTML (KVS/tube players — PimpBunny). Like [`Self::ingest_page`]
    /// and unlike [`Self::execute`], the URL is NOT probed: we already know
    /// it's a real media file (it was in the page as a quality-tagged link),
    /// and probing it server-side tends to fail anyway — these CDNs reject a
    /// bare non-browser request that lacks the page's exact headers/cookies.
    /// It's marked progressive so the planner routes it to the direct HTTP
    /// downloader (with the page as `Referer`) rather than yt-dlp.
    pub fn ingest_direct(&self, meta: DetectionMeta, media_url: &str) -> Option<MediaSource> {
        let url = MediaUrl::parse(media_url).ok()?;
        let source = apply_meta(MediaSource::new(url, MediaFormat::ProgressiveMp4), meta);
        self.events.publish(DomainEvent::MediaDetected {
            source: source.clone(),
        });
        Some(source)
    }
}

fn apply_meta(mut source: MediaSource, meta: DetectionMeta) -> MediaSource {
    if let Some(title) = meta.title.filter(|t| !t.trim().is_empty()) {
        source = source.with_title(title);
    }
    if let Some(thumbnail) = meta.thumbnail.filter(|t| !t.trim().is_empty()) {
        source = source.with_thumbnail(thumbnail);
    }
    if let Some(duration_secs) = meta.duration_secs {
        source = source.with_duration_secs(duration_secs);
    }
    if let Some(tab_id) = meta.source_tab_id {
        source = source.with_browsing_context(tab_id, meta.page_url);
    }
    source
}
