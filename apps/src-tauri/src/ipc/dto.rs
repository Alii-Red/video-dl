//! Wire DTOs mirroring domain types, with `ts-rs` TypeScript generation.
//!
//! UUIDs and timestamps are serialized as strings for a friendlier TS surface.
//! Mappers convert domain → DTO (the IPC layer never leaks domain types).

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::application::ports::{PlaylistEntry, ResolvedPlaylist};
use crate::domain::entities::{Download, HistoryEntry, MediaSource, Task, TaskState};
use crate::domain::value_objects::{
    MediaFormat, Progress, Quality, Settings, WindowGeometry, WindowMode,
};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../frontend/src/types/", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum MediaFormatDto {
    ProgressiveMp4,
    WebM,
    Mov,
    Hls,
    Dash,
}

impl From<MediaFormat> for MediaFormatDto {
    fn from(f: MediaFormat) -> Self {
        match f {
            MediaFormat::ProgressiveMp4 => MediaFormatDto::ProgressiveMp4,
            MediaFormat::WebM => MediaFormatDto::WebM,
            MediaFormat::Mov => MediaFormatDto::Mov,
            MediaFormat::Hls => MediaFormatDto::Hls,
            MediaFormat::Dash => MediaFormatDto::Dash,
        }
    }
}

impl From<MediaFormatDto> for MediaFormat {
    fn from(f: MediaFormatDto) -> Self {
        match f {
            MediaFormatDto::ProgressiveMp4 => MediaFormat::ProgressiveMp4,
            MediaFormatDto::WebM => MediaFormat::WebM,
            MediaFormatDto::Mov => MediaFormat::Mov,
            MediaFormatDto::Hls => MediaFormat::Hls,
            MediaFormatDto::Dash => MediaFormat::Dash,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, TS)]
#[ts(export, export_to = "../frontend/src/types/", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum TaskStateDto {
    Queued,
    Running,
    Paused,
    Completed,
    Failed,
    Cancelled,
}

impl From<TaskState> for TaskStateDto {
    fn from(s: TaskState) -> Self {
        match s {
            TaskState::Queued => TaskStateDto::Queued,
            TaskState::Running => TaskStateDto::Running,
            TaskState::Paused => TaskStateDto::Paused,
            TaskState::Completed => TaskStateDto::Completed,
            TaskState::Failed => TaskStateDto::Failed,
            TaskState::Cancelled => TaskStateDto::Cancelled,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export, export_to = "../frontend/src/types/", rename_all = "camelCase")]
#[serde(rename_all = "camelCase")]
pub struct QualityDto {
    pub height: Option<u32>,
    pub bitrate: Option<u64>,
    pub codec: Option<String>,
    pub has_audio: bool,
}

impl From<Quality> for QualityDto {
    fn from(q: Quality) -> Self {
        Self {
            height: q.height,
            bitrate: q.bitrate,
            codec: q.codec,
            has_audio: q.has_audio,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, TS)]
#[ts(export, export_to = "../frontend/src/types/", rename_all = "camelCase")]
#[serde(rename_all = "camelCase")]
pub struct ProgressDto {
    pub percent: f32,
    pub speed_bps: Option<u64>,
    pub eta_secs: Option<u64>,
}

impl From<Progress> for ProgressDto {
    fn from(p: Progress) -> Self {
        Self {
            percent: p.percent,
            speed_bps: p.speed_bps,
            eta_secs: p.eta_secs,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export, export_to = "../frontend/src/types/", rename_all = "camelCase")]
#[serde(rename_all = "camelCase")]
pub struct MediaSourceDto {
    pub url: String,
    pub format: MediaFormatDto,
    pub title: Option<String>,
    pub qualities: Vec<QualityDto>,
    pub protected: bool,
    pub thumbnail: Option<String>,
    pub duration_secs: Option<u32>,
    /// Origin browser tab/page, when detected live via the extension. `None`
    /// for every other source — the widget only scopes items that have this.
    pub source_tab_id: Option<u32>,
    pub page_url: Option<String>,
}

impl From<MediaSource> for MediaSourceDto {
    fn from(s: MediaSource) -> Self {
        Self {
            url: s.url.as_str().to_string(),
            format: s.format.into(),
            title: s.title,
            qualities: s.qualities.into_iter().map(QualityDto::from).collect(),
            protected: s.protected,
            thumbnail: s.thumbnail,
            duration_secs: s.duration_secs,
            source_tab_id: s.source_tab_id,
            page_url: s.page_url,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export, export_to = "../frontend/src/types/", rename_all = "camelCase")]
#[serde(rename_all = "camelCase")]
pub struct DownloadDto {
    pub id: String,
    pub source_url: String,
    pub provider_id: String,
    pub title: Option<String>,
    pub format: MediaFormatDto,
    pub output_path: Option<String>,
    pub created_at: String,
}

impl From<Download> for DownloadDto {
    fn from(d: Download) -> Self {
        Self {
            id: d.id.to_string(),
            source_url: d.source_url.as_str().to_string(),
            provider_id: d.provider_id,
            title: d.title,
            format: d.format.into(),
            output_path: d.output_path,
            created_at: d.created_at.to_rfc3339(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export, export_to = "../frontend/src/types/", rename_all = "camelCase")]
#[serde(rename_all = "camelCase")]
pub struct TaskDto {
    pub id: String,
    pub download_id: String,
    pub state: TaskStateDto,
    pub progress: ProgressDto,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<Task> for TaskDto {
    fn from(t: Task) -> Self {
        Self {
            id: t.id.to_string(),
            download_id: t.download_id.to_string(),
            state: t.state.into(),
            progress: t.progress.into(),
            error_code: t.error_code,
            error_message: t.error_message,
            created_at: t.created_at.to_rfc3339(),
            updated_at: t.updated_at.to_rfc3339(),
        }
    }
}

/// An archived completed download, for the history view.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export, export_to = "../frontend/src/types/", rename_all = "camelCase")]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntryDto {
    pub download_id: String,
    pub title: Option<String>,
    pub source_url: String,
    pub final_path: Option<String>,
    pub bytes: Option<u64>,
    pub completed_at: String,
}

impl From<HistoryEntry> for HistoryEntryDto {
    fn from(h: HistoryEntry) -> Self {
        Self {
            download_id: h.download_id.to_string(),
            title: h.title,
            source_url: h.source_url,
            final_path: h.final_path,
            bytes: h.bytes,
            completed_at: h.completed_at.to_rfc3339(),
        }
    }
}

/// A download joined with its task, for the active downloads view.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export, export_to = "../frontend/src/types/", rename_all = "camelCase")]
#[serde(rename_all = "camelCase")]
pub struct DownloadViewDto {
    pub download: DownloadDto,
    pub task: TaskDto,
}

impl From<(Download, Task)> for DownloadViewDto {
    fn from((download, task): (Download, Task)) -> Self {
        Self {
            download: DownloadDto::from(download),
            task: TaskDto::from(task),
        }
    }
}

// ---- Command inputs / outputs ------------------------------------------------

#[derive(Debug, Clone, PartialEq, Deserialize, TS)]
#[ts(export, export_to = "../frontend/src/types/", rename_all = "camelCase")]
#[serde(rename_all = "camelCase")]
pub struct StartDownloadArgs {
    pub url: String,
    pub provider_id: String,
    pub format: MediaFormatDto,
    pub title: Option<String>,
    /// Per-download quality cap in pixels (height); `null` = best available.
    pub max_height: Option<u32>,
    /// Download audio only (extract/keep the audio track).
    pub audio_only: bool,
    /// The page this was detected on, when known (widget detections carry
    /// their source page) — used as the Referer for direct HTTP downloads.
    pub referer: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export, export_to = "../frontend/src/types/", rename_all = "camelCase")]
#[serde(rename_all = "camelCase")]
pub struct StartDownloadResponse {
    pub download_id: String,
    pub task_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../frontend/src/types/", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum WindowModeDto {
    Standard,
    Volatile,
}

impl From<WindowMode> for WindowModeDto {
    fn from(m: WindowMode) -> Self {
        match m {
            WindowMode::Standard => WindowModeDto::Standard,
            WindowMode::Volatile => WindowModeDto::Volatile,
        }
    }
}

impl From<WindowModeDto> for WindowMode {
    fn from(m: WindowModeDto) -> Self {
        match m {
            WindowModeDto::Standard => WindowMode::Standard,
            WindowModeDto::Volatile => WindowMode::Volatile,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../frontend/src/types/", rename_all = "camelCase")]
#[serde(rename_all = "camelCase")]
pub struct WindowGeometryDto {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl From<WindowGeometry> for WindowGeometryDto {
    fn from(g: WindowGeometry) -> Self {
        Self {
            x: g.x,
            y: g.y,
            width: g.width,
            height: g.height,
        }
    }
}

impl From<WindowGeometryDto> for WindowGeometry {
    fn from(g: WindowGeometryDto) -> Self {
        Self {
            x: g.x,
            y: g.y,
            width: g.width,
            height: g.height,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../frontend/src/types/", rename_all = "camelCase")]
#[serde(rename_all = "camelCase")]
pub struct SettingsDto {
    pub download_dir: String,
    pub concurrency: u32,
    pub max_height: Option<u32>,
    pub audio_only: bool,
    pub subtitles: bool,
    pub rate_limit_bps: Option<u64>,
    pub window_mode: WindowModeDto,
    pub volatile_geometry: Option<WindowGeometryDto>,
    pub extension_enabled: bool,
    pub extension_port: u16,
    pub extension_token: String,
}

impl From<Settings> for SettingsDto {
    fn from(s: Settings) -> Self {
        Self {
            download_dir: s.download_dir,
            concurrency: s.concurrency,
            max_height: s.max_height,
            audio_only: s.audio_only,
            subtitles: s.subtitles,
            rate_limit_bps: s.rate_limit_bps,
            window_mode: s.window_mode.into(),
            volatile_geometry: s.volatile_geometry.map(Into::into),
            extension_enabled: s.extension_enabled,
            extension_port: s.extension_port,
            extension_token: s.extension_token,
        }
    }
}

impl From<SettingsDto> for Settings {
    fn from(s: SettingsDto) -> Self {
        Self {
            download_dir: s.download_dir,
            concurrency: s.concurrency,
            max_height: s.max_height,
            audio_only: s.audio_only,
            subtitles: s.subtitles,
            rate_limit_bps: s.rate_limit_bps,
            window_mode: s.window_mode.into(),
            volatile_geometry: s.volatile_geometry.map(Into::into),
            extension_enabled: s.extension_enabled,
            extension_port: s.extension_port,
            extension_token: s.extension_token,
        }
    }
}

// ---- Playlists ---------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../frontend/src/types/", rename_all = "camelCase")]
#[serde(rename_all = "camelCase")]
pub struct PlaylistEntryDto {
    pub url: String,
    pub title: Option<String>,
    pub duration_secs: Option<u32>,
    pub thumbnail: Option<String>,
}

impl From<PlaylistEntry> for PlaylistEntryDto {
    fn from(e: PlaylistEntry) -> Self {
        Self {
            url: e.url,
            title: e.title,
            duration_secs: e.duration_secs,
            thumbnail: e.thumbnail,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export, export_to = "../frontend/src/types/", rename_all = "camelCase")]
#[serde(rename_all = "camelCase")]
pub struct PlaylistDto {
    pub title: String,
    pub entries: Vec<PlaylistEntryDto>,
}

impl From<ResolvedPlaylist> for PlaylistDto {
    fn from(p: ResolvedPlaylist) -> Self {
        Self {
            title: p.title,
            entries: p.entries.into_iter().map(PlaylistEntryDto::from).collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize, TS)]
#[ts(export, export_to = "../frontend/src/types/", rename_all = "camelCase")]
#[serde(rename_all = "camelCase")]
pub struct StartPlaylistArgs {
    /// The playlist URL as pasted (stored on the playlist record).
    pub source_url: String,
    pub title: String,
    /// The entries the user kept, in playlist order.
    pub entries: Vec<PlaylistEntryDto>,
    pub max_height: Option<u32>,
    pub audio_only: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export, export_to = "../frontend/src/types/", rename_all = "camelCase")]
#[serde(rename_all = "camelCase")]
pub struct StartPlaylistResponse {
    pub playlist_id: String,
    /// One per queued entry, in the same order as `StartPlaylistArgs.entries`.
    pub downloads: Vec<StartDownloadResponse>,
}
