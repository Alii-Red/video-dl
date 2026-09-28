use super::{ytdlp_format_args, Capabilities, DownloadOptions, Provider};

pub struct YouTubeProvider;

impl Provider for YouTubeProvider {
    fn id(&self) -> &'static str {
        "youtube"
    }

    fn matches(&self, url: &str) -> bool {
        let u = url.to_ascii_lowercase();
        u.contains("youtube.com") || u.contains("youtu.be")
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            playlist: true,
            live: true,
            subtitles: true,
            chapters: true,
        }
    }

    fn build_ytdlp_args(&self, _url: &str, options: &DownloadOptions) -> Vec<String> {
        let mut args = ytdlp_format_args(options);
        // YouTube exposes chapter markers; keep them in the output.
        args.push("--embed-chapters".into());
        args
    }
}
