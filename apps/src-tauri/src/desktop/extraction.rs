//! Page extraction: opens a real Tauri window on the target URL with an
//! injected script that hooks fetch/XHR (see
//! `infrastructure/network/inject/hook.js`), and collects the media URLs it
//! reports. This is the Tauri-specific implementation of the
//! [`PageMediaExtractor`] port — the only piece of the extraction feature that
//! needs to know about windows.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder, WindowEvent};
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::application::error::ApplicationError;
use crate::application::ports::{ExtractedCandidate, PageMediaExtractor};

const HOOK_JS: &str = include_str!("../infrastructure/network/inject/hook.js");

/// Maps an extraction window's label to the channel collecting the
/// candidates it reports. Shared between `TauriPageExtractor` (which creates
/// entries) and the `report_detected_url` command (which the injected script
/// calls into).
#[derive(Default)]
pub struct ExtractorRegistry {
    sessions: Mutex<HashMap<String, mpsc::UnboundedSender<ExtractedCandidate>>>,
}

impl ExtractorRegistry {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    fn insert(&self, label: String, tx: mpsc::UnboundedSender<ExtractedCandidate>) {
        self.sessions.lock().unwrap().insert(label, tx);
    }

    fn remove(&self, label: &str) {
        self.sessions.lock().unwrap().remove(label);
    }

    /// Route a candidate reported by the injected script to the session
    /// whose window label matches. Public: the `#[tauri::command]` wrapper
    /// for this lives in `desktop::mod` (macro-generated helpers don't cross
    /// module boundaries cleanly), so it just calls this.
    pub fn report(
        &self,
        label: &str,
        url: String,
        title: Option<String>,
        thumbnail: Option<String>,
        duration_secs: Option<u32>,
    ) {
        if let Some(tx) = self.sessions.lock().unwrap().get(label) {
            let _ = tx.send(ExtractedCandidate {
                url,
                title,
                thumbnail,
                duration_secs,
            });
        }
    }
}

pub struct TauriPageExtractor {
    app: AppHandle,
    registry: Arc<ExtractorRegistry>,
}

impl TauriPageExtractor {
    pub fn new(app: AppHandle, registry: Arc<ExtractorRegistry>) -> Self {
        Self { app, registry }
    }

    fn open_window(&self, label: &str, url: &str, visible: bool) -> Result<(), ApplicationError> {
        let parsed = url
            .parse()
            .map_err(|e| ApplicationError::Probe(format!("invalid page url: {e}")))?;
        WebviewWindowBuilder::new(&self.app, label, WebviewUrl::External(parsed))
            .initialization_script(HOOK_JS)
            .visible(visible)
            .title("Video DL — extraction")
            .build()
            .map_err(|e| ApplicationError::Probe(format!("failed to open extraction window: {e}")))?;
        Ok(())
    }
}

#[async_trait]
impl PageMediaExtractor for TauriPageExtractor {
    async fn extract_silent(
        &self,
        url: &str,
        timeout: Duration,
    ) -> Result<Vec<ExtractedCandidate>, ApplicationError> {
        let label = format!("extract-{}", Uuid::new_v4());
        let (tx, mut rx) = mpsc::unbounded_channel();
        self.registry.insert(label.clone(), tx);

        self.open_window(&label, url, false)?;

        let mut collected = Vec::new();
        let deadline = tokio::time::sleep(timeout);
        tokio::pin!(deadline);
        loop {
            tokio::select! {
                _ = &mut deadline => break,
                maybe = rx.recv() => match maybe {
                    Some(u) => collected.push(u),
                    None => break,
                },
            }
        }

        self.registry.remove(&label);
        if let Some(window) = self.app.get_webview_window(&label) {
            let _ = window.close();
        }
        Ok(collected)
    }

    async fn extract_interactive(
        &self,
        url: &str,
    ) -> Result<mpsc::UnboundedReceiver<ExtractedCandidate>, ApplicationError> {
        let label = format!("extract-{}", Uuid::new_v4());
        let (tx, rx) = mpsc::unbounded_channel();
        self.registry.insert(label.clone(), tx);

        self.open_window(&label, url, true)?;

        // Dropping the sender when the window closes ends the channel, which
        // makes the use case's `rx.recv()` loop return `None` and finish.
        let registry = self.registry.clone();
        let label_for_close = label.clone();
        if let Some(window) = self.app.get_webview_window(&label) {
            window.on_window_event(move |event| {
                if matches!(event, WindowEvent::Destroyed) {
                    registry.remove(&label_for_close);
                }
            });
        }

        Ok(rx)
    }
}
