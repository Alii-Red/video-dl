//! Direct HTTP download engine: streams a plain file URL to disk instead of
//! handing it to yt-dlp. Exists for hosts (tube/KVS players like PimpBunny)
//! whose direct file link is a real, already-probed `video/mp4` (etc) that
//! yt-dlp's generic extractor refuses on its own safety heuristics (e.g. an
//! unusual extension after a redirect) — no extraction needed here, just a
//! GET with the right `Referer`.

use std::path::PathBuf;
use std::time::Instant;

use async_trait::async_trait;
use futures_util::StreamExt;
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc;

use crate::application::error::ApplicationError;
use crate::application::ports::{DownloadEngine, EngineOutcome, EngineRequest};
use crate::domain::value_objects::Progress;

use super::ytdlp_engine::sanitize_filename;

pub struct HttpDownloadEngine {
    client: reqwest::Client,
}

impl HttpDownloadEngine {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .user_agent(concat!("VideoDL/", env!("CARGO_PKG_VERSION")))
            .build()
            .expect("failed to build reqwest client");
        Self { client }
    }
}

impl Default for HttpDownloadEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl DownloadEngine for HttpDownloadEngine {
    async fn run(
        &self,
        request: EngineRequest,
        progress: mpsc::Sender<Progress>,
    ) -> Result<EngineOutcome, ApplicationError> {
        let url = request
            .direct_url
            .as_deref()
            .ok_or_else(|| ApplicationError::Engine("no direct_url on request".into()))?;

        let mut req = self.client.get(url);
        if let Some(referer) = &request.referer {
            // A Referer never carries a URL fragment; the page URL may have one
            // appended for the widget's per-episode scoping (`#vdl:…`), so trim
            // it before sending the header.
            let referer = referer.split('#').next().unwrap_or(referer);
            req = req.header(reqwest::header::REFERER, referer);
            if let Ok(parsed) = reqwest::Url::parse(referer) {
                req = req.header(reqwest::header::ORIGIN, parsed.origin().ascii_serialization());
            }
        }
        let response = req
            .send()
            .await
            .map_err(|e| ApplicationError::Engine(format!("request failed: {e}")))?;
        if !response.status().is_success() {
            return Err(ApplicationError::Engine(format!(
                "server returned {}",
                response.status()
            )));
        }

        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);
        let total = response.content_length();
        let ext = extension_for(url, content_type.as_deref());

        let name = request
            .title
            .as_deref()
            .filter(|t| !t.trim().is_empty())
            .map(sanitize_filename)
            .unwrap_or_else(|| filename_from_url(url));
        let final_path = unique_path(&request.output_dir, &name, &ext);
        let part_path = {
            let mut p = final_path.clone();
            p.set_extension(format!("{ext}.part"));
            p
        };

        let mut file = tokio::fs::File::create(&part_path)
            .await
            .map_err(|e| ApplicationError::Engine(format!("creating output file: {e}")))?;

        let mut stream = response.bytes_stream();
        let mut downloaded: u64 = 0;
        let started = Instant::now();
        let mut last_report = Instant::now();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| ApplicationError::Engine(format!("stream error: {e}")))?;
            file.write_all(&chunk)
                .await
                .map_err(|e| ApplicationError::Engine(format!("writing output file: {e}")))?;
            downloaded += chunk.len() as u64;

            // Throttle progress events to ~5/s — a raw per-chunk send would
            // flood the event bus/DB writes for a fast local connection.
            if last_report.elapsed().as_millis() >= 200 {
                last_report = Instant::now();
                let _ = progress.send(build_progress(downloaded, total, started)).await;
            }
        }
        file.flush()
            .await
            .map_err(|e| ApplicationError::Engine(format!("flushing output file: {e}")))?;
        drop(file);
        let _ = progress.send(build_progress(downloaded, total, started)).await;

        tokio::fs::rename(&part_path, &final_path)
            .await
            .map_err(|e| ApplicationError::Engine(format!("finalizing output file: {e}")))?;

        Ok(EngineOutcome {
            output_path: Some(final_path.to_string_lossy().to_string()),
        })
    }
}

fn build_progress(downloaded: u64, total: Option<u64>, started: Instant) -> Progress {
    let percent = match total {
        Some(t) if t > 0 => (downloaded as f32 / t as f32 * 100.0).min(100.0),
        _ => 0.0,
    };
    let elapsed = started.elapsed().as_secs_f64().max(0.001);
    let speed_bps = Some((downloaded as f64 / elapsed) as u64);
    let eta_secs = match (total, speed_bps) {
        (Some(t), Some(s)) if s > 0 && t > downloaded => Some((t - downloaded) / s),
        _ => None,
    };
    Progress {
        percent,
        speed_bps,
        eta_secs,
    }
}

/// Extension from the URL's path (works for KVS-style `.../586179_720p.mp4/`
/// URLs — trailing slash and all), falling back to the Content-Type.
fn extension_for(url: &str, content_type: Option<&str>) -> String {
    const KNOWN: &[&str] = &["mp4", "m4v", "webm", "mov", "mkv", "m4a", "mp3"];
    if let Ok(parsed) = reqwest::Url::parse(url) {
        let path = parsed.path().trim_end_matches('/');
        if let Some(ext) = path.rsplit('.').next() {
            let ext = ext.to_ascii_lowercase();
            if KNOWN.contains(&ext.as_str()) {
                return ext;
            }
        }
    }
    match content_type.map(|c| c.split(';').next().unwrap_or("").trim().to_ascii_lowercase()) {
        Some(ref ct) if ct == "video/mp4" => "mp4",
        Some(ref ct) if ct == "video/webm" => "webm",
        Some(ref ct) if ct == "video/quicktime" => "mov",
        _ => "mp4",
    }
    .to_string()
}

fn filename_from_url(url: &str) -> String {
    reqwest::Url::parse(url)
        .ok()
        .and_then(|u| {
            u.path_segments()
                .and_then(|mut s| s.rfind(|seg: &&str| !seg.is_empty()))
                .map(|s| s.to_string())
        })
        .filter(|s| !s.is_empty())
        .map(|s| sanitize_filename(&s))
        .unwrap_or_else(|| "download".to_string())
}

/// Avoid clobbering an existing file of the same name.
fn unique_path(dir: &str, name: &str, ext: &str) -> PathBuf {
    let base = PathBuf::from(dir);
    let mut candidate = base.join(format!("{name}.{ext}"));
    let mut n = 1;
    while candidate.exists() {
        candidate = base.join(format!("{name} ({n}).{ext}"));
        n += 1;
    }
    candidate
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extension_for_kvs_trailing_slash_url() {
        assert_eq!(
            extension_for("https://x/get_file/1/2/586179_720p.mp4/", None),
            "mp4"
        );
    }

    #[test]
    fn extension_for_falls_back_to_content_type() {
        assert_eq!(
            extension_for("https://x/remote_control.php?a=1", Some("video/webm")),
            "webm"
        );
    }

    #[test]
    fn extension_for_defaults_to_mp4() {
        assert_eq!(extension_for("https://x/remote_control.php?a=1", None), "mp4");
    }

    #[test]
    fn filename_from_url_uses_last_segment() {
        assert_eq!(
            filename_from_url("https://x/get_file/1/2/586179_720p.mp4/"),
            "586179_720p.mp4"
        );
    }
}
