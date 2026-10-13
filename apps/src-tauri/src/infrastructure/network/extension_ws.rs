//! Local WebSocket bridge (ADR 4.6): receives live media detections pushed by
//! the Opera GX extension and feeds them into the same probe → `MediaDetected`
//! pipeline as every other detection source. Bound to `127.0.0.1` only, with a
//! shared token checked at the WebSocket handshake.

use std::collections::HashMap;
use std::io::Cursor;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::handshake::server::{ErrorResponse, Request, Response};
use tokio_tungstenite::tungstenite::http::StatusCode;
use tokio_tungstenite::tungstenite::Message;

/// A `TcpStream` with some bytes already read from it, which replays that prefix
/// before delegating to the socket. Lets us peek the start of the HTTP request
/// (to answer `GET /config`) and still hand an untouched byte stream to the WS
/// handshake for a real upgrade.
struct Rewind {
    prefix: Cursor<Vec<u8>>,
    inner: TcpStream,
}

impl AsyncRead for Rewind {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let pos = self.prefix.position() as usize;
        let data = self.prefix.get_ref();
        if pos < data.len() {
            let n = std::cmp::min(data.len() - pos, buf.remaining());
            buf.put_slice(&data[pos..pos + n]);
            self.prefix.set_position((pos + n) as u64);
            return Poll::Ready(Ok(()));
        }
        Pin::new(&mut self.inner).poll_read(cx, buf)
    }
}

impl AsyncWrite for Rewind {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        Pin::new(&mut self.inner).poll_write(cx, buf)
    }
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }
    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

use crate::application::error::ApplicationError;
use crate::application::use_cases::{DetectionMeta, IngestDetection};

/// Outbound side of the WS bridge: lets the app push a command back to the
/// connected extension(s). Cheaply cloneable; shared with the Tauri command
/// layer via app state. Held connections drop out of the map on disconnect.
#[derive(Clone, Default)]
pub struct ExtensionWsHandle {
    conns: Arc<Mutex<HashMap<u64, mpsc::UnboundedSender<Message>>>>,
}

impl ExtensionWsHandle {
    fn register(&self, sender: mpsc::UnboundedSender<Message>) -> u64 {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        self.conns.lock().unwrap().insert(id, sender);
        id
    }

    fn unregister(&self, id: u64) {
        self.conns.lock().unwrap().remove(&id);
    }

    /// Whether at least one extension is currently connected over the WS
    /// bridge — drives the widget's live connection indicator.
    pub fn is_connected(&self) -> bool {
        !self.conns.lock().unwrap().is_empty()
    }

    /// Ask the extension to re-scan the active tab now (widget refresh button).
    /// Best-effort: dead senders are simply ignored (they'll be pruned on their
    /// own disconnect).
    pub fn request_rescan(&self) {
        let msg = Message::Text(r#"{"cmd":"rescan"}"#.to_string());
        for sender in self.conns.lock().unwrap().values() {
            let _ = sender.send(msg.clone());
        }
    }
}

/// A message from the extension over the WS channel — either a detected
/// media URL, or a notice that the active tab/page changed (tab switch or
/// navigation), which the widget uses to scope its display (ADR 4.6
/// follow-up). Tagged on `kind` so both share the one connection.
#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
enum InboundMessage {
    Detection {
        #[serde(rename = "mediaUrl")]
        media_url: String,
        #[serde(rename = "tabTitle")]
        tab_title: Option<String>,
        /// Poster/preview image URL, captured by the extension's content
        /// script from the page (`<video poster>` or `og:image`) — best-effort.
        #[serde(rename = "posterUrl")]
        poster_url: Option<String>,
        /// Video duration in whole seconds, if known to the content script
        /// at observation time.
        #[serde(rename = "durationSecs")]
        duration_secs: Option<u32>,
        #[serde(rename = "tabId")]
        tab_id: u32,
        #[serde(rename = "pageUrl")]
        page_url: Option<String>,
    },
    Context {
        #[serde(rename = "tabId")]
        tab_id: u32,
        #[serde(rename = "pageUrl")]
        page_url: Option<String>,
    },
    /// A page the extension recognized as yt-dlp-native (YouTube, ...) — the
    /// page URL itself is the downloadable item, not any sniffed stream.
    Page {
        #[serde(rename = "pageUrl")]
        page_url: String,
        #[serde(rename = "tabId")]
        tab_id: u32,
        title: Option<String>,
    },
    /// A direct file link the extension pulled from a page's HTML (KVS/tube
    /// players). Not probed — downloaded straight over HTTP with the page as
    /// Referer (see `IngestDetection::ingest_direct`).
    Direct {
        #[serde(rename = "mediaUrl")]
        media_url: String,
        #[serde(rename = "tabId")]
        tab_id: u32,
        #[serde(rename = "pageUrl")]
        page_url: Option<String>,
        title: Option<String>,
        #[serde(rename = "posterUrl")]
        poster_url: Option<String>,
        #[serde(rename = "durationSecs")]
        duration_secs: Option<u32>,
    },
}

pub struct ExtensionWsBridge {
    ingest: Arc<IngestDetection>,
    port: u16,
    token: String,
    handle: ExtensionWsHandle,
}

impl ExtensionWsBridge {
    pub fn new(ingest: Arc<IngestDetection>, port: u16, token: String) -> Self {
        Self {
            ingest,
            port,
            token,
            handle: ExtensionWsHandle::default(),
        }
    }

    /// The outbound handle — clone it before `serve()` (which consumes `self`)
    /// to keep a way to push commands (rescan) back to the extension.
    pub fn handle(&self) -> ExtensionWsHandle {
        self.handle.clone()
    }

    /// Run the accept loop forever. Fails only if the port can't be bound.
    pub async fn serve(self) -> Result<(), ApplicationError> {
        let listener = TcpListener::bind(("127.0.0.1", self.port))
            .await
            .map_err(|e| ApplicationError::Probe(format!("WS bind 127.0.0.1:{}: {e}", self.port)))?;

        let port = self.port;
        loop {
            let (stream, _addr) = match listener.accept().await {
                Ok(x) => x,
                Err(_) => continue, // transient accept error; keep serving
            };
            let ingest = self.ingest.clone();
            let token = self.token.clone();
            let handle = self.handle.clone();
            tokio::spawn(async move {
                let _ = handle_connection(stream, port, ingest, token, handle).await;
            });
        }
    }
}

async fn handle_connection(
    mut stream: TcpStream,
    port: u16,
    ingest: Arc<IngestDetection>,
    token: String,
    handle: ExtensionWsHandle,
) -> Result<(), ApplicationError> {
    // Peek the HTTP request headers so we can answer `GET /config` (the
    // extension's zero-config auto-discovery) without disturbing a real WS
    // upgrade. We read up to the end of the header block, then either reply with
    // JSON or replay the bytes into the WS handshake via `Rewind`.
    let mut head = Vec::with_capacity(1024);
    let mut tmp = [0u8; 1024];
    loop {
        match stream.read(&mut tmp).await {
            Ok(0) => break, // EOF
            Ok(n) => {
                head.extend_from_slice(&tmp[..n]);
                if head.windows(4).any(|w| w == b"\r\n\r\n") || head.len() > 16384 {
                    break;
                }
            }
            Err(_) => return Ok(()),
        }
    }
    let head_str = String::from_utf8_lossy(&head);
    let request_line = head_str.lines().next().unwrap_or("");
    let is_ws = head_str.to_ascii_lowercase().contains("upgrade: websocket");
    if !is_ws && request_line.starts_with("GET /config") {
        // Localhost-only config handshake. No CORS headers, so a web page can't
        // read the response cross-origin — only the extension (host perms) and
        // native processes can. The token stays secret from web pages.
        let body = format!("{{\"port\":{port},\"token\":\"{token}\"}}");
        let resp = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        let _ = stream.write_all(resp.as_bytes()).await;
        let _ = stream.flush().await;
        return Ok(());
    }

    // Not a config request — replay the peeked bytes into the WS handshake.
    let stream = Rewind {
        prefix: Cursor::new(head),
        inner: stream,
    };

    // Token check at the handshake: extension connects to `ws://127.0.0.1:PORT/?token=…`.
    // An empty configured token means "no auth" (dev convenience only).
    let expected = token.clone();
    // Err-variant size is dictated by tungstenite's handshake callback signature, not by us.
    #[allow(clippy::result_large_err)]
    let callback = move |req: &Request, resp: Response| -> Result<Response, ErrorResponse> {
        if expected.is_empty()
            || req
                .uri()
                .query()
                .map(|q| query_has_token(q, &expected))
                .unwrap_or(false)
        {
            Ok(resp)
        } else {
            let deny = Response::builder()
                .status(StatusCode::UNAUTHORIZED)
                .body(Some("invalid token".to_string()))
                .expect("static response");
            Err(deny)
        }
    };

    let ws = tokio_tungstenite::accept_hdr_async(stream, callback)
        .await
        .map_err(|e| ApplicationError::Probe(format!("WS handshake: {e}")))?;

    // Split so the app can push commands (rescan) to the extension on the same
    // socket the extension pushes detections on. A dedicated task drains an
    // mpsc into the write half; the read loop below stays as-is.
    let (mut write, mut read) = ws.split();
    let (out_tx, mut out_rx) = mpsc::unbounded_channel::<Message>();
    let conn_id = handle.register(out_tx);
    let writer = tokio::spawn(async move {
        while let Some(msg) = out_rx.recv().await {
            if write.send(msg).await.is_err() {
                break;
            }
        }
    });

    while let Some(msg) = read.next().await {
        let msg = match msg {
            Ok(m) => m,
            Err(_) => break,
        };
        match msg {
            Message::Text(text) => {
                match serde_json::from_str::<InboundMessage>(&text) {
                    Ok(InboundMessage::Detection {
                        media_url,
                        tab_title,
                        poster_url,
                        duration_secs,
                        tab_id,
                        page_url,
                    }) => {
                        // Fire-and-forget: a bad URL just yields None, doesn't
                        // drop the connection.
                        let meta = DetectionMeta {
                            title: tab_title,
                            thumbnail: poster_url,
                            duration_secs,
                            source_tab_id: Some(tab_id),
                            page_url,
                        };
                        ingest.execute(&media_url, meta).await;
                    }
                    Ok(InboundMessage::Context { tab_id, page_url }) => {
                        ingest.publish_context_change(tab_id, page_url);
                    }
                    Ok(InboundMessage::Page {
                        page_url,
                        tab_id,
                        title,
                    }) => {
                        ingest.ingest_page(&page_url, tab_id, title);
                    }
                    Ok(InboundMessage::Direct {
                        media_url,
                        tab_id,
                        page_url,
                        title,
                        poster_url,
                        duration_secs,
                    }) => {
                        let meta = DetectionMeta {
                            title,
                            thumbnail: poster_url,
                            duration_secs,
                            source_tab_id: Some(tab_id),
                            page_url,
                        };
                        ingest.ingest_direct(meta, &media_url);
                    }
                    Err(_) => {}
                }
            }
            Message::Close(_) => break,
            _ => {}
        }
    }
    handle.unregister(conn_id);
    writer.abort();
    Ok(())
}

fn query_has_token(query: &str, expected: &str) -> bool {
    query.split('&').any(|kv| {
        let mut parts = kv.splitn(2, '=');
        parts.next() == Some("token") && parts.next() == Some(expected)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handle_delivers_rescan_to_registered_conns_and_prunes_dropped_ones() {
        let handle = ExtensionWsHandle::default();
        let (tx, mut rx) = mpsc::unbounded_channel();
        let id = handle.register(tx);

        handle.request_rescan();
        assert_eq!(
            rx.try_recv().unwrap(),
            Message::Text(r#"{"cmd":"rescan"}"#.to_string())
        );

        // After unregister, no more delivery (and no panic on an empty map).
        handle.unregister(id);
        handle.request_rescan();
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn query_token_matching() {
        assert!(query_has_token("token=abc", "abc"));
        assert!(query_has_token("x=1&token=abc&y=2", "abc"));
        assert!(!query_has_token("token=abc", "def"));
        assert!(!query_has_token("notoken=abc", "abc"));
    }

    #[test]
    fn detection_message_parses_camel_case() {
        let m: InboundMessage = serde_json::from_str(
            r#"{"kind":"detection","mediaUrl":"https://x/v.m3u8","tabTitle":"T","tabId":7}"#,
        )
        .unwrap();
        match m {
            InboundMessage::Detection {
                media_url,
                tab_title,
                poster_url,
                duration_secs,
                tab_id,
                page_url,
            } => {
                assert_eq!(media_url, "https://x/v.m3u8");
                assert_eq!(tab_title.as_deref(), Some("T"));
                assert_eq!(poster_url, None);
                assert_eq!(duration_secs, None);
                assert_eq!(tab_id, 7);
                assert_eq!(page_url, None);
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn detection_message_parses_poster_and_duration() {
        let m: InboundMessage = serde_json::from_str(
            r#"{"kind":"detection","mediaUrl":"https://x/v.m3u8","posterUrl":"https://x/poster.jpg","durationSecs":6844,"tabId":1,"pageUrl":"https://p"}"#,
        )
        .unwrap();
        match m {
            InboundMessage::Detection {
                poster_url,
                duration_secs,
                page_url,
                ..
            } => {
                assert_eq!(poster_url.as_deref(), Some("https://x/poster.jpg"));
                assert_eq!(duration_secs, Some(6844));
                assert_eq!(page_url.as_deref(), Some("https://p"));
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn context_message_parses() {
        let m: InboundMessage = serde_json::from_str(
            r#"{"kind":"context","tabId":7,"pageUrl":"https://site.example.com/ep16"}"#,
        )
        .unwrap();
        match m {
            InboundMessage::Context { tab_id, page_url } => {
                assert_eq!(tab_id, 7);
                assert_eq!(page_url.as_deref(), Some("https://site.example.com/ep16"));
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn page_message_parses() {
        let m: InboundMessage = serde_json::from_str(
            r#"{"kind":"page","pageUrl":"https://www.youtube.com/watch?v=abc","tabId":3,"title":"Some Video"}"#,
        )
        .unwrap();
        match m {
            InboundMessage::Page {
                page_url,
                tab_id,
                title,
            } => {
                assert_eq!(page_url, "https://www.youtube.com/watch?v=abc");
                assert_eq!(tab_id, 3);
                assert_eq!(title.as_deref(), Some("Some Video"));
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn direct_message_parses() {
        let m: InboundMessage = serde_json::from_str(
            r#"{"kind":"direct","mediaUrl":"https://pb/get_file/1/2/x_1080p.mp4/","tabId":5,"pageUrl":"https://pb/videos/x/","title":"X"}"#,
        )
        .unwrap();
        match m {
            InboundMessage::Direct {
                media_url,
                tab_id,
                page_url,
                title,
                ..
            } => {
                assert_eq!(media_url, "https://pb/get_file/1/2/x_1080p.mp4/");
                assert_eq!(tab_id, 5);
                assert_eq!(page_url.as_deref(), Some("https://pb/videos/x/"));
                assert_eq!(title.as_deref(), Some("X"));
            }
            other => panic!("unexpected: {other:?}"),
        }
    }
}
