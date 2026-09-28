use serde::{Deserialize, Serialize};

/// Window presentation mode. Purely a presentation concern (ADR 1.8) — it does
/// not affect the domain, but it lives in `Settings` because it must persist
/// across restarts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum WindowMode {
    /// Full window: sidebar + all views (the existing layout).
    #[default]
    Standard,
    /// Compact always-on-top widget: detections + active downloads only.
    Volatile,
}

/// Remembered geometry of the volatile widget, so it reopens where the user
/// left it. Logical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WindowGeometry {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Eq for WindowGeometry {}

/// Application settings. A value object persisted as a whole (see the
/// `SettingsRepository` port).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Destination directory for downloads (empty = not yet chosen).
    pub download_dir: String,
    /// Maximum number of simultaneous downloads.
    pub concurrency: u32,
    pub max_height: Option<u32>,
    pub audio_only: bool,
    pub subtitles: bool,
    /// Optional global rate limit in bytes per second.
    pub rate_limit_bps: Option<u64>,
    /// Last window mode used, restored at boot.
    pub window_mode: WindowMode,
    /// Remembered volatile-widget geometry, `None` until first placed.
    pub volatile_geometry: Option<WindowGeometry>,
    /// Whether the local WebSocket bridge (for the Opera GX extension) is on.
    pub extension_enabled: bool,
    /// Port the WS bridge listens on (127.0.0.1 only).
    pub extension_port: u16,
    /// Shared token the extension must present in its handshake. Generated once.
    pub extension_token: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            download_dir: String::new(),
            concurrency: 2,
            max_height: None,
            audio_only: false,
            subtitles: false,
            rate_limit_bps: None,
            window_mode: WindowMode::Standard,
            volatile_geometry: None,
            extension_enabled: true,
            extension_port: 8787,
            extension_token: String::new(),
        }
    }
}
