use super::{ytdlp_format_args, Capabilities, DownloadOptions, Provider};

pub struct RedditProvider;

impl Provider for RedditProvider {
    fn id(&self) -> &'static str {
        "reddit"
    }

    fn matches(&self, url: &str) -> bool {
        let u = url.to_ascii_lowercase();
        u.contains("reddit.com") || u.contains("v.redd.it")
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            playlist: false,
            live: false,
            subtitles: false,
            chapters: false,
        }
    }

    fn build_ytdlp_args(&self, _url: &str, options: &DownloadOptions) -> Vec<String> {
        // Reddit serves separate audio/video (DASH); yt-dlp + FFmpeg merge them.
        ytdlp_format_args(options)
    }
}
