//! Download engines: yt-dlp process driver + progress parsing, a direct HTTP
//! streamer for hosts yt-dlp's generic extractor refuses, and a composite
//! that routes between them.

mod composite_engine;
mod http_download_engine;
mod progress_parser;
mod ytdlp_engine;
mod ytdlp_playlist;

pub use composite_engine::CompositeEngine;
pub use http_download_engine::HttpDownloadEngine;
pub use progress_parser::{parse_progress_line, PROGRESS_TEMPLATE};
pub use ytdlp_engine::YtDlpEngine;
pub use ytdlp_playlist::{looks_like_playlist, parse_flat_playlist, YtDlpPlaylistResolver};
