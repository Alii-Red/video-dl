use std::path::PathBuf;
use std::time::Duration;

use async_trait::async_trait;
use serde_json::Value;
use tokio::process::Command;

use crate::application::error::ApplicationError;
use crate::application::ports::{PlaylistEntry, PlaylistResolver, ResolvedPlaylist};

/// Expands playlists with `yt-dlp --flat-playlist -J`: one fast metadata
/// request per page of the playlist, no per-video extraction, nothing
/// downloaded.
pub struct YtDlpPlaylistResolver {
    ytdlp_path: PathBuf,
    js_runtime: Option<String>,
    timeout: Duration,
}

/// Large channels can take a while to page through, but a wedged process must
/// not leave the UI spinning forever.
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(90);

impl YtDlpPlaylistResolver {
    pub fn new(ytdlp_path: impl Into<PathBuf>, js_runtime: Option<String>) -> Self {
        Self {
            ytdlp_path: ytdlp_path.into(),
            js_runtime,
            timeout: DEFAULT_TIMEOUT,
        }
    }
}

/// Whether a URL is worth handing to yt-dlp as a playlist. Cheap pre-filter so
/// that every pasted single-video link doesn't pay for a yt-dlp spawn.
pub fn looks_like_playlist(url: &str) -> bool {
    let u = url.to_ascii_lowercase();
    let youtube = u.contains("youtube.com") || u.contains("youtu.be");
    if youtube {
        return u.contains("list=")
            || u.contains("/playlist")
            || u.contains("/@")
            || u.contains("/channel/")
            || u.contains("/c/")
            || u.contains("/user/");
    }
    if u.contains("vimeo.com") {
        return u.contains("/showcase/") || u.contains("/album/") || u.contains("/channels/");
    }
    false
}

/// Parse yt-dlp's `-J --flat-playlist` output. `None` when the JSON describes a
/// single video (or an empty playlist). Channel tabs nest playlists inside
/// playlists (Videos / Shorts / Live); those are flattened.
pub fn parse_flat_playlist(json: &str) -> Result<Option<ResolvedPlaylist>, ApplicationError> {
    let root: Value = serde_json::from_str(json)
        .map_err(|e| ApplicationError::Probe(format!("invalid yt-dlp JSON: {e}")))?;
    if root.get("_type").and_then(Value::as_str) != Some("playlist") {
        return Ok(None);
    }
    let mut entries = Vec::new();
    collect_entries(&root, &mut entries);
    if entries.is_empty() {
        return Ok(None);
    }
    let title = root
        .get("title")
        .and_then(Value::as_str)
        .filter(|t| !t.trim().is_empty())
        .unwrap_or("Playlist")
        .to_string();
    Ok(Some(ResolvedPlaylist { title, entries }))
}

fn collect_entries(node: &Value, out: &mut Vec<PlaylistEntry>) {
    let Some(entries) = node.get("entries").and_then(Value::as_array) else {
        return;
    };
    for entry in entries {
        if entry.get("_type").and_then(Value::as_str) == Some("playlist") {
            collect_entries(entry, out);
            continue;
        }
        // Flat entries carry `url` (sometimes just the bare id); prefer the
        // canonical `webpage_url` when present.
        let Some(url) = entry
            .get("webpage_url")
            .or_else(|| entry.get("url"))
            .and_then(Value::as_str)
            .filter(|u| u.starts_with("http"))
        else {
            continue;
        };
        // Deleted/private videos stay listed with a placeholder title and are
        // guaranteed to fail — skip them rather than queue dead downloads.
        let title = entry.get("title").and_then(Value::as_str).map(str::to_string);
        if matches!(title.as_deref(), Some("[Deleted video]" | "[Private video]")) {
            continue;
        }
        if out.iter().any(|e| e.url == url) {
            continue;
        }
        out.push(PlaylistEntry {
            url: url.to_string(),
            title,
            duration_secs: entry
                .get("duration")
                .and_then(Value::as_f64)
                .map(|d| d.round() as u32),
            thumbnail: entry
                .get("thumbnails")
                .and_then(Value::as_array)
                .and_then(|t| t.last())
                .and_then(|t| t.get("url"))
                .and_then(Value::as_str)
                .map(str::to_string),
        });
    }
}

#[async_trait]
impl PlaylistResolver for YtDlpPlaylistResolver {
    async fn resolve(&self, url: &str) -> Result<Option<ResolvedPlaylist>, ApplicationError> {
        if !looks_like_playlist(url) {
            return Ok(None);
        }
        let mut cmd = Command::new(&self.ytdlp_path);
        cmd.args(["--flat-playlist", "--yes-playlist", "-J", "--no-warnings"]);
        if let Some(runtime) = &self.js_runtime {
            cmd.args(["--js-runtimes", runtime]);
        }
        cmd.arg(url).kill_on_drop(true);
        #[cfg(windows)]
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW

        let output = tokio::time::timeout(self.timeout, cmd.output())
            .await
            .map_err(|_| ApplicationError::Probe("playlist resolution timed out".into()))?
            .map_err(|e| ApplicationError::Probe(format!("failed to run yt-dlp: {e}")))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let msg = stderr.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("");
            return Err(ApplicationError::Probe(format!("yt-dlp: {msg}")));
        }
        parse_flat_playlist(&String::from_utf8_lossy(&output.stdout))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefilter_only_accepts_playlist_shaped_urls() {
        assert!(looks_like_playlist("https://www.youtube.com/playlist?list=PL123"));
        assert!(looks_like_playlist("https://www.youtube.com/watch?v=abc&list=PL123"));
        assert!(looks_like_playlist("https://www.youtube.com/@SomeChannel/videos"));
        assert!(looks_like_playlist("https://vimeo.com/showcase/42"));
        assert!(!looks_like_playlist("https://www.youtube.com/watch?v=abc"));
        assert!(!looks_like_playlist("https://youtu.be/abc"));
        assert!(!looks_like_playlist("https://cdn.example.com/video.m3u8"));
    }

    #[test]
    fn single_video_json_is_not_a_playlist() {
        let json = r#"{"_type":"video","id":"abc","title":"One"}"#;
        assert_eq!(parse_flat_playlist(json).unwrap(), None);
    }

    #[test]
    fn parses_flat_playlist_entries() {
        let json = r#"{
            "_type": "playlist", "title": "My list",
            "entries": [
                {"_type":"url","url":"https://www.youtube.com/watch?v=a","title":"A","duration":61.4,
                 "thumbnails":[{"url":"https://i/small.jpg"},{"url":"https://i/big.jpg"}]},
                {"_type":"url","url":"https://www.youtube.com/watch?v=b","title":"[Deleted video]"},
                {"_type":"url","url":"https://www.youtube.com/watch?v=c","title":null},
                {"_type":"url","url":"https://www.youtube.com/watch?v=a","title":"A again"}
            ]
        }"#;
        let pl = parse_flat_playlist(json).unwrap().unwrap();
        assert_eq!(pl.title, "My list");
        assert_eq!(pl.entries.len(), 2);
        assert_eq!(pl.entries[0].title.as_deref(), Some("A"));
        assert_eq!(pl.entries[0].duration_secs, Some(61));
        assert_eq!(pl.entries[0].thumbnail.as_deref(), Some("https://i/big.jpg"));
        assert_eq!(pl.entries[1].url, "https://www.youtube.com/watch?v=c");
        assert_eq!(pl.entries[1].title, None);
    }

    #[test]
    fn flattens_nested_channel_tabs() {
        let json = r#"{
            "_type": "playlist", "title": "Channel",
            "entries": [
                {"_type":"playlist","title":"Videos","entries":[
                    {"url":"https://www.youtube.com/watch?v=1"}]},
                {"_type":"playlist","title":"Shorts","entries":[
                    {"url":"https://www.youtube.com/shorts/2"}]}
            ]
        }"#;
        let pl = parse_flat_playlist(json).unwrap().unwrap();
        assert_eq!(pl.entries.len(), 2);
    }

    #[test]
    fn empty_playlist_is_none() {
        let json = r#"{"_type":"playlist","title":"Empty","entries":[]}"#;
        assert_eq!(parse_flat_playlist(json).unwrap(), None);
    }
}
