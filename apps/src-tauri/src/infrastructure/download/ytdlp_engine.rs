use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use async_trait::async_trait;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;

use crate::application::error::ApplicationError;
use crate::application::ports::{DownloadEngine, EngineOutcome, EngineRequest};
use crate::domain::value_objects::Progress;

use super::progress_parser::{parse_progress_line, PROGRESS_TEMPLATE};

/// Drives the external `yt-dlp` binary. `kill_on_drop` means dropping the run
/// future (e.g. on cancel) terminates the process.
pub struct YtDlpEngine {
    ytdlp_path: PathBuf,
    /// Directory containing the FFmpeg binary (passed to yt-dlp for merging).
    ffmpeg_dir: Option<PathBuf>,
    /// Optional global download rate limit, in bytes per second.
    rate_limit_bps: Option<u64>,
    /// JavaScript runtime spec for yt-dlp's `--js-runtimes` (e.g. `"node"` or
    /// `"node:C:\\Program Files\\nodejs\\node.exe"`). Modern yt-dlp needs a JS
    /// runtime to solve YouTube's signature challenge — without one, YouTube
    /// extraction fails outright ("Video unavailable").
    js_runtime: Option<String>,
    /// Parallel HLS/DASH fragment downloads (`--concurrent-fragments`).
    concurrent_fragments: u8,
    /// Kill the process if it produces no stdout for this long — guards
    /// against a yt-dlp that hangs (network stall, wedged extractor) leaving
    /// the task stuck in "running" forever.
    inactivity_timeout: Duration,
}

/// Generous enough that a slow-but-alive extraction/merge never trips it,
/// while still bounding a truly wedged process.
const DEFAULT_INACTIVITY_TIMEOUT: Duration = Duration::from_secs(180);
const DEFAULT_CONCURRENT_FRAGMENTS: u8 = 4;

impl YtDlpEngine {
    pub fn new(ytdlp_path: impl Into<PathBuf>, ffmpeg_dir: Option<PathBuf>) -> Self {
        Self {
            ytdlp_path: ytdlp_path.into(),
            ffmpeg_dir,
            rate_limit_bps: None,
            js_runtime: None,
            concurrent_fragments: DEFAULT_CONCURRENT_FRAGMENTS,
            inactivity_timeout: DEFAULT_INACTIVITY_TIMEOUT,
        }
    }

    /// Override the parallel-fragment count (`--concurrent-fragments`).
    pub fn with_concurrent_fragments(mut self, n: u8) -> Self {
        self.concurrent_fragments = n;
        self
    }

    /// Override the stdout-inactivity timeout.
    pub fn with_inactivity_timeout(mut self, timeout: Duration) -> Self {
        self.inactivity_timeout = timeout;
        self
    }

    /// Cap the download rate (bytes/second). `None` means unlimited.
    pub fn with_rate_limit(mut self, rate_limit_bps: Option<u64>) -> Self {
        self.rate_limit_bps = rate_limit_bps;
        self
    }

    /// Set the JS runtime spec passed to yt-dlp (`--js-runtimes`). Required
    /// for YouTube; harmless for other sites.
    pub fn with_js_runtime(mut self, js_runtime: Option<String>) -> Self {
        self.js_runtime = js_runtime;
        self
    }

    /// Assemble the full yt-dlp argument list. Provider-specific args (format
    /// selection, etc.) come first, then our common flags. Pure — unit tested.
    pub fn build_args(&self, req: &EngineRequest) -> Vec<String> {
        let mut args: Vec<String> = Vec::new();
        args.extend(req.ytdlp_args.iter().cloned());
        args.push("--newline".into());
        args.push("--color".into());
        args.push("never".into());
        args.push("--progress-template".into());
        args.push(PROGRESS_TEMPLATE.into());
        args.push("--continue".into());
        // Download only the single video, never a playlist/mix. YouTube "watch"
        // URLs opened from a Mix or radio carry a `list=RD…&start_radio=1` query
        // — without this, yt-dlp treats that list as a playlist and grabs the
        // whole (effectively endless) mix one entry at a time, which looks like
        // a "slow, stuttering" download for what should be one short clip. This
        // tool is single-video by design (playlists are a separate, deferred
        // feature), so pin it off unconditionally.
        args.push("--no-playlist".into());
        args.push("--merge-output-format".into());
        args.push("mp4".into());
        // Prefer an mp4/m4a (AAC) audio track over the "best" one, which on
        // YouTube is Opus. Opus inside an mp4 container plays silently in most
        // players (Windows Media Player, etc.) — the video looks fine but has
        // no sound. Sorting by `ext:mp4:m4a` keeps the best *video* (AV1/H264)
        // while pinning the audio to AAC, so the merged mp4 has audible sound
        // everywhere without transcoding (which the bundled minimal FFmpeg
        // can't do — it has no Opus decoder).
        args.push("-S".into());
        args.push("ext:mp4:m4a".into());
        // Download several HLS/DASH fragments at once — a big speed-up on
        // fragmented streams (YouTube, X/Twitter, anime-sama...). Ignored for
        // plain progressive files, so it's always safe to pass.
        args.push("--concurrent-fragments".into());
        args.push(self.concurrent_fragments.max(1).to_string());
        if let Some(rate) = self.rate_limit_bps {
            args.push("--limit-rate".into());
            args.push(rate.to_string());
        }
        if let Some(dir) = &self.ffmpeg_dir {
            args.push("--ffmpeg-location".into());
            args.push(dir.display().to_string());
        }
        if let Some(runtime) = &self.js_runtime {
            args.push("--js-runtimes".into());
            args.push(runtime.clone());
        }
        // Without an explicit title, yt-dlp names the file after whatever
        // metadata it can extract — for a raw HLS/DASH URL with no matching
        // site extractor, that's often the manifest's own filename,
        // including its full (often signed, token-bearing) query string.
        if let Some(title) = req.title.as_deref().filter(|t| !t.trim().is_empty()) {
            let mut name = sanitize_filename(title);
            // Streaming sites (anime-sama, ...) give every episode of a series
            // the SAME page title ("Dragon Ball Z - Saison 1 | Anime-Sama"), so
            // deriving the filename from the title alone makes every episode
            // collide on one name. yt-dlp then sees the existing file, reports
            // "already downloaded", and finishes instantly without fetching the
            // new episode — the download appears to vanish with no result. When
            // the source is an HLS/DASH manifest, append a short stable hash of
            // the manifest's PATH (episode-unique; the rotating auth token lives
            // in the query, which we exclude so re-detecting the same episode
            // keeps the same name and --continue can still resume it).
            if let Some(key) = manifest_disambiguator(&req.url) {
                name = format!("{name} [{key}]");
            }
            args.push("-o".into());
            args.push(format!("{}.%(ext)s", name));
        }
        args.push("-P".into());
        args.push(req.output_dir.clone());
        args.push(req.url.clone());
        args
    }
}

/// Read one line from an async byte stream, decoding it leniently
/// (`String::from_utf8_lossy`) instead of requiring strict UTF-8. Unlike
/// `AsyncBufReadExt::lines()`, a single malformed byte (e.g. a path segment
/// yt-dlp printed in the Windows console's ANSI codepage instead of UTF-8)
/// becomes a replacement character rather than aborting the whole read —
/// and, by extension, killing an otherwise-healthy download. Returns `None`
/// at EOF or on a genuine I/O error.
async fn read_line_lossy<R: tokio::io::AsyncRead + Unpin>(
    reader: &mut BufReader<R>,
) -> Option<String> {
    let mut buf = Vec::new();
    match reader.read_until(b'\n', &mut buf).await {
        Ok(0) => None,
        Ok(_) => {
            while matches!(buf.last(), Some(b'\n' | b'\r')) {
                buf.pop();
            }
            Some(String::from_utf8_lossy(&buf).into_owned())
        }
        Err(_) => None,
    }
}

/// Strip characters illegal in Windows filenames (`< > : " / \ | ? *` and
/// control characters), collapse whitespace, and trim trailing dots/spaces
/// (Windows silently drops them, which can otherwise produce a filename that
/// doesn't match what yt-dlp reports). Capped well under Windows' MAX_PATH so
/// the full path (download dir + name + extension) stays valid.
/// For an HLS/DASH manifest URL, a short stable hex key derived from its path
/// (query excluded, so a re-signed re-detection of the same stream maps to the
/// same key). Returns `None` for non-manifest URLs (YouTube pages, direct mp4s,
/// ...), which already carry their own uniqueness in the title/id and shouldn't
/// get a hash suffix. Used to keep same-titled episodes from colliding on one
/// output filename.
pub(crate) fn manifest_disambiguator(url: &str) -> Option<String> {
    use std::hash::{Hash, Hasher};
    // Path is everything before the query string.
    let path = url.split(['?', '#']).next().unwrap_or(url);
    let lower = path.to_ascii_lowercase();
    if !(lower.contains(".m3u8") || lower.contains(".mpd")) {
        return None;
    }
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    path.hash(&mut hasher);
    Some(format!("{:08x}", hasher.finish() as u32))
}

pub(crate) fn sanitize_filename(raw: &str) -> String {
    let cleaned: String = raw
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => ' ',
            c if c.is_control() => ' ',
            c => c,
        })
        .collect();
    let collapsed = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    let trimmed = collapsed.trim_matches(|c: char| c == '.' || c == ' ');
    let capped: String = trimmed.chars().take(150).collect();
    if capped.is_empty() {
        "download".into()
    } else {
        capped
    }
}

#[async_trait]
impl DownloadEngine for YtDlpEngine {
    async fn run(
        &self,
        request: EngineRequest,
        progress: mpsc::Sender<Progress>,
    ) -> Result<EngineOutcome, ApplicationError> {
        // Have yt-dlp write the final file path to a sidecar temp file, so we can
        // report `output_path` robustly (no stdout parsing).
        let path_file = std::env::temp_dir().join(format!("vdl-path-{}.txt", uuid::Uuid::new_v4()));
        let mut args = self.build_args(&request);
        args.push("--no-simulate".into());
        args.push("--print-to-file".into());
        args.push("after_move:filepath".into());
        args.push(path_file.to_string_lossy().to_string());

        let mut cmd = Command::new(&self.ytdlp_path);
        cmd.args(&args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            // yt-dlp (Python) picks its stdout encoding from the Windows
            // console codepage unless told otherwise, and will happily print
            // paths containing non-ASCII characters (an accented username in
            // the destination path, for example) in that codepage rather
            // than UTF-8. Force UTF-8 so those bytes are always well-formed.
            .env("PYTHONUTF8", "1")
            .env("PYTHONIOENCODING", "utf-8");
        #[cfg(windows)]
        {
            // Prevent a console window from flashing up around the child
            // process (CREATE_NO_WINDOW = 0x08000000).
            cmd.creation_flags(0x0800_0000);
        }

        let mut child = cmd
            .spawn()
            .map_err(|e| ApplicationError::Engine(format!("failed to spawn yt-dlp: {e}")))?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| ApplicationError::Engine("yt-dlp stdout unavailable".into()))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| ApplicationError::Engine("yt-dlp stderr unavailable".into()))?;

        // Drain stderr concurrently so the pipe never blocks; keep it for errors.
        let stderr_handle = tokio::spawn(async move {
            let mut collected = String::new();
            let mut reader = BufReader::new(stderr);
            while let Some(line) = read_line_lossy(&mut reader).await {
                collected.push_str(&line);
                collected.push('\n');
            }
            collected
        });

        let mut stdout_reader = BufReader::new(stdout);
        loop {
            match tokio::time::timeout(
                self.inactivity_timeout,
                read_line_lossy(&mut stdout_reader),
            )
            .await
            {
                Ok(Some(line)) => {
                    if let Some(p) = parse_progress_line(&line) {
                        // Receiver may have gone away (task cancelled) — fine.
                        let _ = progress.send(p).await;
                    }
                }
                Ok(None) => break, // EOF — process finished normally.
                Err(_elapsed) => {
                    // No output for `inactivity_timeout`: treat as wedged.
                    // Returning here drops `child`, and `kill_on_drop` kills
                    // the yt-dlp process; the stderr task then hits EOF.
                    stderr_handle.abort();
                    return Err(ApplicationError::Engine(format!(
                        "yt-dlp inactif pendant {}s — téléchargement interrompu",
                        self.inactivity_timeout.as_secs()
                    )));
                }
            }
        }

        let status = child
            .wait()
            .await
            .map_err(|e| ApplicationError::Engine(format!("waiting on yt-dlp: {e}")))?;
        let stderr_text = stderr_handle.await.unwrap_or_default();

        if status.success() {
            let output_path = std::fs::read_to_string(&path_file)
                .ok()
                .and_then(|s| s.lines().next().map(|l| l.trim().to_string()))
                .filter(|s| !s.is_empty());
            let _ = std::fs::remove_file(&path_file);
            Ok(EngineOutcome { output_path })
        } else {
            let _ = std::fs::remove_file(&path_file);
            Err(ApplicationError::Engine(concise_error(&stderr_text)))
        }
    }
}

/// Distill yt-dlp's stderr into a short, human-readable reason for the UI.
/// yt-dlp prints a stream of warnings then (usually) one `ERROR:` line — that
/// line is what actually matters. Prefer it; strip yt-dlp's `ERROR:` and
/// `[extractor] id:` noise; fall back to the last non-empty line. Truncated
/// so a runaway message can't bloat the DB row / event payload.
fn concise_error(stderr: &str) -> String {
    let pick = stderr
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .rfind(|l| l.to_ascii_uppercase().contains("ERROR"))
        .or_else(|| stderr.lines().map(str::trim).rfind(|l| !l.is_empty()))
        .unwrap_or("le téléchargement a échoué");

    // Drop a leading "ERROR:" and any "[extractor] videoid:" prefix.
    let cleaned = pick
        .trim_start_matches("ERROR:")
        .trim()
        .trim_start_matches('[')
        .split_once("] ")
        .map(|(_, rest)| rest)
        .unwrap_or_else(|| pick.trim_start_matches("ERROR:").trim());
    // yt-dlp often prefixes "videoid: " after the extractor tag; keep it,
    // it's short. Just cap the length.
    let cleaned = cleaned.trim();
    if cleaned.chars().count() > 300 {
        cleaned.chars().take(297).collect::<String>() + "…"
    } else {
        cleaned.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req() -> EngineRequest {
        EngineRequest {
            url: "https://example.com/v.mp4".into(),
            output_dir: "C:/downloads".into(),
            ytdlp_args: vec!["-f".into(), "bestvideo+bestaudio".into()],
            title: None,
            direct_url: None,
            referer: None,
        }
    }

    #[test]
    fn build_args_puts_provider_args_first_and_url_last() {
        let engine = YtDlpEngine::new("yt-dlp", None);
        let args = engine.build_args(&req());
        assert_eq!(args[0], "-f");
        assert_eq!(args[1], "bestvideo+bestaudio");
        assert_eq!(args.last().unwrap(), "https://example.com/v.mp4");
        assert!(args.contains(&"--continue".to_string()));
        assert!(args.contains(&"--newline".to_string()));
    }

    #[test]
    fn build_args_includes_ffmpeg_location_when_set() {
        let engine = YtDlpEngine::new("yt-dlp", Some(PathBuf::from("C:/bin")));
        let args = engine.build_args(&req());
        let i = args.iter().position(|a| a == "--ffmpeg-location").unwrap();
        assert_eq!(args[i + 1], "C:/bin");
    }

    #[test]
    fn build_args_omits_ffmpeg_location_when_unset() {
        let engine = YtDlpEngine::new("yt-dlp", None);
        assert!(!engine
            .build_args(&req())
            .contains(&"--ffmpeg-location".to_string()));
    }

    #[test]
    fn build_args_includes_rate_limit_when_set() {
        let engine = YtDlpEngine::new("yt-dlp", None).with_rate_limit(Some(500_000));
        let args = engine.build_args(&req());
        let i = args.iter().position(|a| a == "--limit-rate").unwrap();
        assert_eq!(args[i + 1], "500000");
    }

    #[test]
    fn build_args_prefers_mp4_aac_audio() {
        // Guards against silent Opus-in-mp4 output on YouTube.
        let args = YtDlpEngine::new("yt-dlp", None).build_args(&req());
        let i = args.iter().position(|a| a == "-S").unwrap();
        assert_eq!(args[i + 1], "ext:mp4:m4a");
    }

    #[test]
    fn build_args_disables_playlist() {
        // A YouTube Mix/radio watch URL carries list=RD…; without --no-playlist
        // yt-dlp would download the entire (endless) mix instead of one clip.
        let args = YtDlpEngine::new("yt-dlp", None).build_args(&req());
        assert!(args.contains(&"--no-playlist".to_string()));
    }

    #[test]
    fn build_args_sets_concurrent_fragments() {
        let args = YtDlpEngine::new("yt-dlp", None)
            .with_concurrent_fragments(6)
            .build_args(&req());
        let i = args.iter().position(|a| a == "--concurrent-fragments").unwrap();
        assert_eq!(args[i + 1], "6");
    }

    #[test]
    fn build_args_includes_js_runtime_when_set() {
        let engine =
            YtDlpEngine::new("yt-dlp", None).with_js_runtime(Some("node:C:/node.exe".into()));
        let args = engine.build_args(&req());
        let i = args.iter().position(|a| a == "--js-runtimes").unwrap();
        assert_eq!(args[i + 1], "node:C:/node.exe");
    }

    #[test]
    fn build_args_omits_js_runtime_when_unset() {
        let engine = YtDlpEngine::new("yt-dlp", None);
        assert!(!engine
            .build_args(&req())
            .contains(&"--js-runtimes".to_string()));
    }

    #[test]
    fn concise_error_picks_the_error_line_and_strips_noise() {
        let stderr = "WARNING: [youtube] No supported JavaScript runtime could be found.\n\
                      [youtube] abc: Downloading webpage\n\
                      ERROR: [youtube] abc123: Video unavailable\n";
        assert_eq!(concise_error(stderr), "abc123: Video unavailable");
    }

    #[test]
    fn concise_error_falls_back_to_last_line_without_error_tag() {
        let stderr = "some spurious output\nunexpected end of stream\n";
        assert_eq!(concise_error(stderr), "unexpected end of stream");
    }

    #[test]
    fn concise_error_handles_empty_stderr() {
        assert_eq!(concise_error("   \n\n"), "le téléchargement a échoué");
    }

    #[test]
    fn build_args_omits_rate_limit_by_default() {
        let engine = YtDlpEngine::new("yt-dlp", None);
        assert!(!engine.build_args(&req()).contains(&"--limit-rate".to_string()));
    }

    #[test]
    fn build_args_uses_sanitized_title_as_output_template_when_present() {
        let engine = YtDlpEngine::new("yt-dlp", None);
        let mut request = req();
        request.title = Some("Dragon Ball Z: Kai / Episode 1?".into());
        let args = engine.build_args(&request);
        let i = args.iter().position(|a| a == "-o").unwrap();
        assert_eq!(args[i + 1], "Dragon Ball Z Kai Episode 1.%(ext)s");
    }

    #[test]
    fn build_args_omits_output_template_without_title() {
        let engine = YtDlpEngine::new("yt-dlp", None);
        assert!(!engine.build_args(&req()).contains(&"-o".to_string()));
    }

    #[test]
    fn build_args_omits_output_template_for_blank_title() {
        let engine = YtDlpEngine::new("yt-dlp", None);
        let mut request = req();
        request.title = Some("   ".into());
        assert!(!engine.build_args(&request).contains(&"-o".to_string()));
    }

    #[test]
    fn sanitize_filename_strips_illegal_windows_characters() {
        assert_eq!(
            sanitize_filename(r#"a<b>c:d"e/f\g|h?i*j"#),
            "a b c d e f g h i j"
        );
    }

    #[test]
    fn sanitize_filename_trims_trailing_dots_and_spaces() {
        assert_eq!(sanitize_filename("  My Video...  "), "My Video");
    }

    #[test]
    fn sanitize_filename_falls_back_when_empty_after_cleanup() {
        assert_eq!(sanitize_filename("???"), "download");
    }

    #[test]
    fn sanitize_filename_caps_length() {
        let long = "a".repeat(300);
        assert_eq!(sanitize_filename(&long).len(), 150);
    }

    #[test]
    fn manifest_disambiguator_only_for_manifests() {
        assert!(manifest_disambiguator("https://youtube.com/watch?v=abc").is_none());
        assert!(manifest_disambiguator("https://x/get_file/1/v_1080p.mp4/").is_none());
        assert!(manifest_disambiguator("https://cdn/hls/ep265/master.m3u8?t=tok").is_some());
        assert!(manifest_disambiguator("https://cdn/dash/index.mpd").is_some());
    }

    #[test]
    fn manifest_disambiguator_ignores_query_but_differs_by_path() {
        // Same episode, re-signed token in the query → same key (so --continue
        // resumes the same file).
        let a = manifest_disambiguator("https://cdn/hls/6t8g3k01xzgg/master.m3u8?t=AAA&s=1");
        let b = manifest_disambiguator("https://cdn/hls/6t8g3k01xzgg/master.m3u8?t=BBB&s=2");
        assert_eq!(a, b);
        // Different episode (different path id) → different key.
        let c = manifest_disambiguator("https://cdn/hls/ZZZZZZZZZZZZ/master.m3u8?t=AAA&s=1");
        assert_ne!(a, c);
    }

    #[test]
    fn build_args_disambiguates_manifest_output_name() {
        // A titled HLS download gets a [hash] suffix so same-titled episodes
        // don't collide on one filename (and silently "already downloaded").
        let engine = YtDlpEngine::new("yt-dlp", None);
        let mut r = req();
        r.url = "https://cdn/hls/ep265/master.m3u8?t=tok".into();
        r.title = Some("Dragon Ball Z - Saison 1".into());
        let args = engine.build_args(&r);
        let i = args.iter().position(|a| a == "-o").unwrap();
        assert!(args[i + 1].starts_with("Dragon Ball Z - Saison 1 ["));
        assert!(args[i + 1].ends_with("].%(ext)s"));
    }
}
