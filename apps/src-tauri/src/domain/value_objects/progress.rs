use serde::{Deserialize, Serialize};

/// Download progress snapshot. `percent` is 0..=100.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Progress {
    pub percent: f32,
    pub speed_bps: Option<u64>,
    pub eta_secs: Option<u64>,
}

impl Progress {
    pub fn zero() -> Self {
        Self {
            percent: 0.0,
            speed_bps: None,
            eta_secs: None,
        }
    }

    /// Build from a raw percent, clamped into 0..=100.
    pub fn clamped(percent: f32) -> Self {
        Self {
            percent: percent.clamp(0.0, 100.0),
            speed_bps: None,
            eta_secs: None,
        }
    }
}

impl Default for Progress {
    fn default() -> Self {
        Self::zero()
    }
}
