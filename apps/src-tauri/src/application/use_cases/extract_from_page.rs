use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use crate::application::error::ApplicationError;
use crate::application::events::DomainEvent;
use crate::application::ports::{EventPublisher, ExtractedCandidate, MediaProbe, PageMediaExtractor};
use crate::domain::entities::MediaSource;
use crate::domain::value_objects::MediaUrl;

/// Extraction from a regular web page (as opposed to a direct media link):
/// open it in a real browser engine, observe the network requests it makes,
/// and probe each candidate through the same [`MediaProbe`] pipeline used for
/// direct links — so HLS/DASH parsing, protection detection, etc. all apply
/// identically regardless of how the URL was found.
pub struct ExtractFromPage {
    extractor: Arc<dyn PageMediaExtractor>,
    probe: Arc<dyn MediaProbe>,
    events: Arc<dyn EventPublisher>,
}

impl ExtractFromPage {
    pub fn new(
        extractor: Arc<dyn PageMediaExtractor>,
        probe: Arc<dyn MediaProbe>,
        events: Arc<dyn EventPublisher>,
    ) -> Self {
        Self {
            extractor,
            probe,
            events,
        }
    }

    /// Silent pass: hidden window, fixed timeout, batch result. Meant to run
    /// automatically as a fallback right after a direct probe fails.
    pub async fn execute_silent(
        &self,
        page_url: &str,
        timeout: Duration,
    ) -> Result<Vec<MediaSource>, ApplicationError> {
        let candidates = self.extractor.extract_silent(page_url, timeout).await?;
        Ok(self.probe_candidates(candidates, page_url).await)
    }

    /// Interactive pass: visible window, results streamed one at a time as the
    /// user interacts with the page. Publishes a `MediaDetected` event per
    /// find and returns once the user closes the window (channel closes).
    pub async fn execute_interactive(&self, page_url: &str) -> Result<(), ApplicationError> {
        let mut rx = self.extractor.extract_interactive(page_url).await?;
        let mut seen = HashSet::new();
        while let Some(candidate) = rx.recv().await {
            if !seen.insert(candidate.url.clone()) {
                continue;
            }
            self.probe_one(candidate, page_url).await;
        }
        Ok(())
    }

    /// Probe each unique candidate, publishing an event for every hit.
    /// Individual probe failures (noise like tracker beacons that slipped
    /// past the injected script's prefilter) are dropped rather than failing
    /// the whole batch.
    async fn probe_candidates(
        &self,
        candidates: Vec<ExtractedCandidate>,
        page_url: &str,
    ) -> Vec<MediaSource> {
        let mut seen = HashSet::new();
        let mut sources = Vec::new();
        for candidate in candidates {
            if !seen.insert(candidate.url.clone()) {
                continue;
            }
            if let Some(source) = self.probe_one(candidate, page_url).await {
                sources.push(source);
            }
        }
        sources
    }

    async fn probe_one(&self, candidate: ExtractedCandidate, page_url: &str) -> Option<MediaSource> {
        let url = MediaUrl::parse(&candidate.url).ok()?;
        // The extraction window's own URL is a solid Referer — many CDNs
        // reject requests that don't look like they came from the actual
        // page (see MediaProbe::probe).
        let mut source = self.probe.probe(&url, Some(page_url)).await.ok()?;
        if let Some(title) = candidate.title.filter(|t| !t.trim().is_empty()) {
            source = source.with_title(title);
        }
        if let Some(thumbnail) = candidate.thumbnail.filter(|t| !t.trim().is_empty()) {
            source = source.with_thumbnail(thumbnail);
        }
        if let Some(duration_secs) = candidate.duration_secs {
            source = source.with_duration_secs(duration_secs);
        }
        self.events.publish(DomainEvent::MediaDetected {
            source: source.clone(),
        });
        Some(source)
    }
}
