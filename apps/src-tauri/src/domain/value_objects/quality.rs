use serde::{Deserialize, Serialize};

/// A selectable quality variant of a media source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Quality {
    pub height: Option<u32>,
    pub bitrate: Option<u64>,
    pub codec: Option<String>,
    pub has_audio: bool,
}

impl Quality {
    pub fn new(
        height: Option<u32>,
        bitrate: Option<u64>,
        codec: Option<String>,
        has_audio: bool,
    ) -> Self {
        Self {
            height,
            bitrate,
            codec,
            has_audio,
        }
    }

    /// Ranking key used to pick the "best" quality: resolution dominates, bitrate
    /// breaks ties.
    pub fn rank(&self) -> (u32, u64) {
        (self.height.unwrap_or(0), self.bitrate.unwrap_or(0))
    }
}
