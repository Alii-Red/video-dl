use serde::{Deserialize, Serialize};

/// Supported media container / delivery formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MediaFormat {
    ProgressiveMp4,
    WebM,
    Mov,
    Hls,
    Dash,
}

impl MediaFormat {
    /// True for streaming manifests (as opposed to a single progressive file).
    pub fn is_manifest(&self) -> bool {
        matches!(self, MediaFormat::Hls | MediaFormat::Dash)
    }
}
