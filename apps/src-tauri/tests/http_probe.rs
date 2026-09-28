//! Integration test for `HttpMediaProbe`: verifies the Referer/Origin headers
//! are actually sent when a referer is known — many CDNs (YouTube's
//! googlevideo, adult tube sites' anti-hotlinking guards, ...) silently
//! reject requests missing them, which is why detections were failing
//! without any visible error.

use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use video_dl::application::ports::MediaProbe;
use video_dl::domain::value_objects::MediaUrl;
use video_dl::infrastructure::network::HttpMediaProbe;

/// Accepts one HTTP/1.1 request, returns the raw request text (headers
/// included), and replies with a minimal 200 `video/mp4` response so the
/// probe completes successfully.
async fn serve_one_request_and_capture(listener: TcpListener) -> String {
    let (mut stream, _) = listener.accept().await.unwrap();
    let mut buf = vec![0u8; 8192];
    let n = tokio::time::timeout(Duration::from_secs(2), stream.read(&mut buf))
        .await
        .unwrap()
        .unwrap();
    let request = String::from_utf8_lossy(&buf[..n]).to_string();

    let response = b"HTTP/1.1 200 OK\r\nContent-Type: video/mp4\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
    stream.write_all(response).await.unwrap();
    stream.flush().await.unwrap();
    request
}

#[tokio::test]
async fn probe_sends_referer_and_origin_when_known() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();

    let server = tokio::spawn(serve_one_request_and_capture(listener));

    let probe = HttpMediaProbe::new();
    let url = MediaUrl::parse(&format!("http://127.0.0.1:{port}/video.mp4")).unwrap();
    probe
        .probe(&url, Some("https://site.example.com/watch/ep16"))
        .await
        .expect("probe should succeed against the local server");

    let request = server.await.unwrap();
    assert!(
        request.to_ascii_lowercase().contains("referer: https://site.example.com/watch/ep16"),
        "expected a Referer header, got:\n{request}"
    );
    assert!(
        request.to_ascii_lowercase().contains("origin: https://site.example.com"),
        "expected an Origin header, got:\n{request}"
    );
}

#[tokio::test]
async fn probe_omits_referer_when_none_given() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();

    let server = tokio::spawn(serve_one_request_and_capture(listener));

    let probe = HttpMediaProbe::new();
    let url = MediaUrl::parse(&format!("http://127.0.0.1:{port}/video.mp4")).unwrap();
    probe
        .probe(&url, None)
        .await
        .expect("probe should succeed against the local server");

    let request = server.await.unwrap();
    assert!(
        !request.to_ascii_lowercase().contains("referer:"),
        "did not expect a Referer header, got:\n{request}"
    );
}
