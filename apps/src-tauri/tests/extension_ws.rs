//! Integration test for `ExtensionWsBridge`: a real WebSocket client connects
//! over TCP (as the Opera GX extension would) and pushes detections through
//! to a fake probe, verifying the token gate and the `MediaDetected` pipeline.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use futures_util::SinkExt;
use tokio_tungstenite::tungstenite::Message;

use video_dl::application::error::ApplicationError;
use video_dl::application::events::DomainEvent;
use video_dl::application::ports::{EventPublisher, MediaProbe};
use video_dl::application::use_cases::IngestDetection;
use video_dl::domain::entities::MediaSource;
use video_dl::domain::value_objects::{MediaFormat, MediaUrl};
use video_dl::infrastructure::events::InMemoryEventBus;
use video_dl::infrastructure::network::ExtensionWsBridge;

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

async fn spawn_bridge(token: &str) -> (u16, Arc<InMemoryEventBus>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener); // free the port for the bridge to bind; tiny race, acceptable in tests

    let probe = Arc::new(FakeProbe);
    let events = Arc::new(InMemoryEventBus::new(64));
    let publisher: Arc<dyn EventPublisher> = events.clone();
    let ingest = Arc::new(IngestDetection::new(probe, publisher));
    let bridge = ExtensionWsBridge::new(ingest, port, token.to_string());
    tokio::spawn(async move {
        let _ = bridge.serve().await;
    });
    // Give the accept loop a moment to bind.
    tokio::time::sleep(Duration::from_millis(50)).await;
    (port, events)
}

#[tokio::test]
async fn valid_token_and_message_yields_media_detected_event() {
    let (port, events) = spawn_bridge("secret").await;
    let mut rx = events.subscribe();

    let (mut ws, _resp) =
        tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}/?token=secret"))
            .await
            .expect("handshake should succeed with the correct token");

    ws.send(Message::Text(
        r#"{"kind":"detection","mediaUrl":"https://cdn.example.com/a.mp4","tabUrl":"https://p","tabTitle":"T","tabId":1}"#
            .to_string(),
    ))
    .await
    .unwrap();

    let evt = tokio::time::timeout(Duration::from_secs(2), rx.recv())
        .await
        .expect("event should arrive")
        .unwrap();
    match evt {
        DomainEvent::MediaDetected { source } => {
            assert_eq!(source.url.as_str(), "https://cdn.example.com/a.mp4");
        }
        other => panic!("unexpected event: {other:?}"),
    }
}

#[tokio::test]
async fn poster_and_duration_are_carried_onto_the_media_source() {
    let (port, events) = spawn_bridge("secret").await;
    let mut rx = events.subscribe();

    let (mut ws, _resp) =
        tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}/?token=secret"))
            .await
            .unwrap();

    ws.send(Message::Text(
        r#"{"kind":"detection","mediaUrl":"https://cdn.example.com/a.mp4","posterUrl":"https://cdn.example.com/poster.jpg","durationSecs":6844,"tabId":1,"pageUrl":"https://p/ep16"}"#
            .to_string(),
    ))
    .await
    .unwrap();

    let evt = tokio::time::timeout(Duration::from_secs(2), rx.recv())
        .await
        .expect("event should arrive")
        .unwrap();
    match evt {
        DomainEvent::MediaDetected { source } => {
            assert_eq!(
                source.thumbnail.as_deref(),
                Some("https://cdn.example.com/poster.jpg")
            );
            assert_eq!(source.duration_secs, Some(6844));
            assert_eq!(source.source_tab_id, Some(1));
            assert_eq!(source.page_url.as_deref(), Some("https://p/ep16"));
        }
        other => panic!("unexpected event: {other:?}"),
    }
}

#[tokio::test]
async fn context_message_publishes_browsing_context_changed() {
    let (port, events) = spawn_bridge("secret").await;
    let mut rx = events.subscribe();

    let (mut ws, _resp) =
        tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}/?token=secret"))
            .await
            .unwrap();

    ws.send(Message::Text(
        r#"{"kind":"context","tabId":7,"pageUrl":"https://site.example.com/ep16"}"#.to_string(),
    ))
    .await
    .unwrap();

    let evt = tokio::time::timeout(Duration::from_secs(2), rx.recv())
        .await
        .expect("event should arrive")
        .unwrap();
    match evt {
        DomainEvent::BrowsingContextChanged { tab_id, page_url } => {
            assert_eq!(tab_id, 7);
            assert_eq!(page_url.as_deref(), Some("https://site.example.com/ep16"));
        }
        other => panic!("unexpected event: {other:?}"),
    }
}

#[tokio::test]
async fn page_message_yields_a_source_without_probing() {
    // A "page" (YouTube-style) is never HTTP-probed — even a probe that would
    // ERROR on the URL must not prevent the source from being published. Use a
    // page URL the FakeProbe would reject to prove the probe isn't consulted.
    let (port, events) = spawn_bridge("secret").await;
    let mut rx = events.subscribe();

    let (mut ws, _resp) =
        tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}/?token=secret"))
            .await
            .unwrap();

    ws.send(Message::Text(
        r#"{"kind":"page","pageUrl":"https://www.youtube.com/watch?v=noise","tabId":9,"title":"A YouTube Video"}"#
            .to_string(),
    ))
    .await
    .unwrap();

    let evt = tokio::time::timeout(Duration::from_secs(2), rx.recv())
        .await
        .expect("event should arrive")
        .unwrap();
    match evt {
        DomainEvent::MediaDetected { source } => {
            assert_eq!(source.url.as_str(), "https://www.youtube.com/watch?v=noise");
            assert_eq!(source.title.as_deref(), Some("A YouTube Video"));
            assert_eq!(source.source_tab_id, Some(9));
            assert_eq!(
                source.page_url.as_deref(),
                Some("https://www.youtube.com/watch?v=noise")
            );
        }
        other => panic!("unexpected event: {other:?}"),
    }
}

#[tokio::test]
async fn direct_message_yields_a_source_without_probing() {
    // A "direct" file (KVS/tube link pulled from the page HTML) is never
    // probed — the FakeProbe would reject "noise", yet it must still be
    // published, with its browsing context set so the widget shows it and
    // the planner can route it to the direct HTTP downloader.
    let (port, events) = spawn_bridge("secret").await;
    let mut rx = events.subscribe();

    let (mut ws, _resp) =
        tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}/?token=secret"))
            .await
            .unwrap();

    ws.send(Message::Text(
        r#"{"kind":"direct","mediaUrl":"https://pb/get_file/1/2/noise_1080p.mp4/","tabId":4,"pageUrl":"https://pb/videos/x/","title":"X"}"#
            .to_string(),
    ))
    .await
    .unwrap();

    let evt = tokio::time::timeout(Duration::from_secs(2), rx.recv())
        .await
        .expect("event should arrive")
        .unwrap();
    match evt {
        DomainEvent::MediaDetected { source } => {
            assert_eq!(source.url.as_str(), "https://pb/get_file/1/2/noise_1080p.mp4/");
            assert_eq!(source.title.as_deref(), Some("X"));
            assert_eq!(source.source_tab_id, Some(4));
            assert_eq!(source.page_url.as_deref(), Some("https://pb/videos/x/"));
        }
        other => panic!("unexpected event: {other:?}"),
    }
}

#[tokio::test]
async fn wrong_token_is_rejected_at_handshake() {
    let (port, _events) = spawn_bridge("secret").await;

    let err = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}/?token=wrong"))
        .await
        .err();

    assert!(err.is_some(), "handshake with the wrong token must fail");
}

#[tokio::test]
async fn bad_probe_url_does_not_crash_the_connection() {
    let (port, events) = spawn_bridge("secret").await;
    let mut rx = events.subscribe();

    let (mut ws, _resp) =
        tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}/?token=secret"))
            .await
            .unwrap();

    ws.send(Message::Text(
        r#"{"kind":"detection","mediaUrl":"https://cdn.example.com/noise.gif","tabId":1}"#
            .to_string(),
    ))
    .await
    .unwrap();
    ws.send(Message::Text(
        r#"{"kind":"detection","mediaUrl":"https://cdn.example.com/good.mp4","tabId":1}"#
            .to_string(),
    ))
    .await
    .unwrap();

    let evt = tokio::time::timeout(Duration::from_secs(2), rx.recv())
        .await
        .expect("the good url should still be published despite the earlier probe failure")
        .unwrap();
    match evt {
        DomainEvent::MediaDetected { source } => {
            assert_eq!(source.url.as_str(), "https://cdn.example.com/good.mp4");
        }
        other => panic!("unexpected event: {other:?}"),
    }
}
