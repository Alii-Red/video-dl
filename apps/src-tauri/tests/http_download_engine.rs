//! Integration test for `HttpDownloadEngine`: serves a small "video" body
//! over a local TCP server (verifying the Referer header, like a KVS/tube
//! host would require) and checks the file actually lands on disk with the
//! right name/extension and reported progress.

use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::mpsc;

use video_dl::application::ports::{DownloadEngine, EngineRequest};
use video_dl::infrastructure::download::HttpDownloadEngine;

/// Accepts one HTTP/1.1 request, captures its raw text, and replies with a
/// `video/mp4` body of `body` bytes.
async fn serve_and_capture(listener: TcpListener, body: &'static [u8]) -> String {
    let (mut stream, _) = listener.accept().await.unwrap();
    let mut buf = vec![0u8; 8192];
    let n = tokio::time::timeout(Duration::from_secs(2), stream.read(&mut buf))
        .await
        .unwrap()
        .unwrap();
    let request = String::from_utf8_lossy(&buf[..n]).to_string();

    let header = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: video/mp4\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(header.as_bytes()).await.unwrap();
    stream.write_all(body).await.unwrap();
    stream.flush().await.unwrap();
    request
}

#[tokio::test]
async fn downloads_a_direct_url_with_referer_and_writes_the_file() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let body: &'static [u8] = b"fake-mp4-bytes-0123456789";

    let server = tokio::spawn(serve_and_capture(listener, body));

    let out_dir = std::env::temp_dir().join(format!("vdl-http-engine-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&out_dir).unwrap();

    let engine = HttpDownloadEngine::new();
    let (tx, mut rx) = mpsc::channel(16);
    let req = EngineRequest {
        url: "unused-for-direct-downloads".into(),
        output_dir: out_dir.to_string_lossy().to_string(),
        ytdlp_args: vec![],
        title: Some("Adria Rae Sucks".into()),
        direct_url: Some(format!("http://127.0.0.1:{port}/get_file/x/586179_720p.mp4/")),
        referer: Some("https://pimpbunny.com/videos/adria-rae/".into()),
    };

    let outcome = engine.run(req, tx).await.expect("download should succeed");
    let request = server.await.unwrap();

    assert!(
        request
            .to_ascii_lowercase()
            .contains("referer: https://pimpbunny.com/videos/adria-rae/"),
        "expected a Referer header, got:\n{request}"
    );

    let path = outcome.output_path.expect("output_path should be set");
    assert!(path.ends_with(".mp4"), "expected an .mp4 file, got: {path}");
    let written = std::fs::read(&path).unwrap();
    assert_eq!(written, body);

    // At least one progress event, ending at (or near) 100%.
    let mut last = None;
    while let Ok(p) = rx.try_recv() {
        last = Some(p);
    }
    let last = last.expect("expected at least one progress event");
    assert!(last.percent >= 99.0, "expected ~100%, got {}", last.percent);

    std::fs::remove_dir_all(&out_dir).ok();
}

#[tokio::test]
async fn errors_when_no_direct_url_is_set() {
    let engine = HttpDownloadEngine::new();
    let (tx, _rx) = mpsc::channel(16);
    let req = EngineRequest {
        url: "https://example.com/x.mp4".into(),
        output_dir: ".".into(),
        ytdlp_args: vec![],
        title: None,
        direct_url: None,
        referer: None,
    };
    let err = engine.run(req, tx).await.unwrap_err();
    assert!(err.to_string().contains("direct_url"));
}
