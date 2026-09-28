use super::{ytdlp_format_args, Capabilities, DownloadOptions, Provider};

/// Universal fallback: delegates everything to yt-dlp with no site-specific
/// logic. Covers the vast majority of sites.
pub struct GenericProvider;

impl Provider for GenericProvider {
    fn id(&self) -> &'static str {
        "generic"
    }

    /// Matches anything — but the registry only uses it as the explicit fallback,
    /// so it never shadows a specific provider.
    fn matches(&self, _url: &str) -> bool {
        true
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
        ytdlp_format_args(options)
    }
}
