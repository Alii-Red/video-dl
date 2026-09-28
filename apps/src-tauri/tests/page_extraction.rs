//! Tests for the `ExtractFromPage` use case with a fake `PageMediaExtractor` —
//! no real window is opened (that piece is Tauri-only, exercised manually).

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::mpsc;

use video_dl::application::error::ApplicationError;
use video_dl::application::events::DomainEvent;
use video_dl::application::ports::{
    EventPublisher, ExtractedCandidate, MediaProbe, PageMediaExtractor,
};
use video_dl::application::use_cases::ExtractFromPage;
use video_dl::domain::entities::MediaSource;
use video_dl::domain::value_objects::{MediaFormat, MediaUrl};
use video_dl::infrastructure::events::InMemoryEventBus;

/// Returns a fixed batch for `extract_silent`; for `extract_interactive`,
/// sends the same candidates one at a time over the returned channel then
/// closes it (simulating the user closing the window after the page loaded
/// its player).
struct FakeExtractor {
    candidates: Vec<ExtractedCandidate>,
}

#[async_trait]
impl PageMediaExtractor for FakeExtractor {
    async fn extract_silent(
        &self,
        _url: &str,
        _timeout: Duration,
    ) -> Result<Vec<ExtractedCandidate>, ApplicationError> {
        Ok(self.candidates.clone())
    }

    async fn extract_interactive(
        &self,
        _url: &str,
    ) -> Result<mpsc::UnboundedReceiver<ExtractedCandidate>, ApplicationError> {
        let (tx, rx) = mpsc::unbounded_channel();
        for c in self.candidates.clone() {
            let _ = tx.send(c);
        }
        // Dropping `tx` here closes the channel, exactly like the real
        // extractor does when the window is destroyed.
        Ok(rx)
    }
}

/// Classifies anything ending in a known media extension as progressive MP4
/// (good enough to exercise the use case's probe→publish plumbing).
struct FakeProbe;

#[async_trait]
impl MediaProbe for FakeProbe {
    async fn probe(&self, url: &MediaUrl, _referer: Option<&str>) -> Result<MediaSource, ApplicationError> {
        if url.as_str().contains("noise") {
            return Err(ApplicationError::Probe("not a media url".into()));
        }
        Ok(MediaSource::new(url.clone(), MediaFormat::ProgressiveMp4))
    }
}

fn use_case(candidates: Vec<&str>) -> (ExtractFromPage, Arc<InMemoryEventBus>) {
    let candidates = candidates
        .into_iter()
        .map(|url| ExtractedCandidate {
            url: url.into(),
            title: None,
            thumbnail: None,
            duration_secs: None,
        })
        .collect();
    use_case_with_candidates(candidates)
}

fn use_case_with_candidates(
    candidates: Vec<ExtractedCandidate>,
) -> (ExtractFromPage, Arc<InMemoryEventBus>) {
    let extractor = Arc::new(FakeExtractor { candidates });
    let probe = Arc::new(FakeProbe);
    let events = Arc::new(InMemoryEventBus::new(64));
    let publisher: Arc<dyn EventPublisher> = events.clone();
    (ExtractFromPage::new(extractor, probe, publisher), events)
}

#[tokio::test]
async fn silent_extraction_probes_and_dedupes_candidates() {
    let (uc, events) = use_case(vec![
        "https://cdn.example.com/a.mp4",
        "https://cdn.example.com/a.mp4", // duplicate, must be collapsed
        "https://cdn.example.com/b.m3u8",
    ]);
    let mut rx = events.subscribe();

    let sources = uc
        .execute_silent("https://site.example.com/watch", Duration::from_secs(1))
        .await
        .unwrap();

    assert_eq!(sources.len(), 2);

    // One MediaDetected event per unique, successfully-probed source.
    let mut seen = 0;
    while let Ok(DomainEvent::MediaDetected { .. }) = rx.try_recv() {
        seen += 1;
    }
    assert_eq!(seen, 2);
}

#[tokio::test]
async fn silent_extraction_drops_urls_that_fail_to_probe() {
    let (uc, _events) = use_case(vec![
        "https://cdn.example.com/good.mp4",
        "https://cdn.example.com/noise.gif", // classify()-passing but probe fails
    ]);

    let sources = uc
        .execute_silent("https://site.example.com/watch", Duration::from_secs(1))
        .await
        .unwrap();

    assert_eq!(sources.len(), 1);
}

#[tokio::test]
async fn silent_extraction_with_no_candidates_returns_empty() {
    let (uc, _events) = use_case(vec![]);
    let sources = uc
        .execute_silent("https://site.example.com/watch", Duration::from_secs(1))
        .await
        .unwrap();
    assert!(sources.is_empty());
}

#[tokio::test]
async fn silent_extraction_carries_the_page_title_onto_the_source() {
    let (uc, _events) = use_case_with_candidates(vec![ExtractedCandidate {
        url: "https://cdn.example.com/a.mp4".into(),
        title: Some("Dragon Ball Z - Kai - Episode 1".into()),
        thumbnail: None,
        duration_secs: None,
    }]);

    let sources = uc
        .execute_silent("https://site.example.com/watch", Duration::from_secs(1))
        .await
        .unwrap();

    assert_eq!(sources.len(), 1);
    assert_eq!(
        sources[0].title.as_deref(),
        Some("Dragon Ball Z - Kai - Episode 1")
    );
}

#[tokio::test]
async fn silent_extraction_ignores_blank_title() {
    let (uc, _events) = use_case_with_candidates(vec![ExtractedCandidate {
        url: "https://cdn.example.com/a.mp4".into(),
        title: Some("   ".into()),
        thumbnail: None,
        duration_secs: None,
    }]);

    let sources = uc
        .execute_silent("https://site.example.com/watch", Duration::from_secs(1))
        .await
        .unwrap();

    assert_eq!(sources[0].title, None);
}

#[tokio::test]
async fn interactive_extraction_publishes_as_it_goes_and_ends_when_channel_closes() {
    let (uc, events) = use_case(vec![
        "https://cdn.example.com/a.mp4",
        "https://cdn.example.com/a.mp4", // duplicate
        "https://cdn.example.com/b.mp4",
    ]);
    let mut rx = events.subscribe();

    // Resolves once the fake extractor's channel closes (its tx is dropped
    // after sending), mirroring the user closing the interactive window.
    uc.execute_interactive("https://site.example.com/watch")
        .await
        .unwrap();

    let mut seen = 0;
    while let Ok(DomainEvent::MediaDetected { .. }) = rx.try_recv() {
        seen += 1;
    }
    assert_eq!(seen, 2, "duplicate should not produce a second event");
}
