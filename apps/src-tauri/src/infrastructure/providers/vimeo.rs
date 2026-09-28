use super::{ytdlp_format_args, Capabilities, DownloadOptions, Provider};

pub struct VimeoProvider;

impl Provider for VimeoProvider {
    fn id(&self) -> &'static str {
        "vimeo"
    }

    fn matches(&self, url: &str) -> bool {
        url.to_ascii_lowercase().contains("vimeo.com")
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            playlist: true,
            live: false,
            subtitles: true,
            chapters: false,
        }
    }

    fn build_ytdlp_args(&self, _url: &str, options: &DownloadOptions) -> Vec<String> {
        ytdlp_format_args(options)
    }
}
