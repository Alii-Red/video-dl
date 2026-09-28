use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::value_objects::{MediaFormat, MediaUrl};

/// A requested download — the "what". Its runtime state lives in [`super::Task`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Download {
    pub id: Uuid,
    pub source_url: MediaUrl,
    pub provider_id: String,
    pub title: Option<String>,
    pub format: MediaFormat,
    pub output_path: Option<String>,
    pub created_at: DateTime<Utc>,
    /// Per-download quality cap in pixels (height), chosen at download time.
    /// `None` = best available (or fall back to the global default).
    pub max_height: Option<u32>,
    /// Per-download "audio only" choice, chosen at download time.
    pub audio_only: bool,
    /// The page this was detected on, when known — sent as the
    /// Referer/Origin header for a direct HTTP download (see
    /// `RegistryPlanner`); many hosts reject a hotlinked GET without it.
    pub referer: Option<String>,
}

impl Download {
    pub fn new(source_url: MediaUrl, provider_id: impl Into<String>, format: MediaFormat) -> Self {
        Self {
            id: Uuid::new_v4(),
            source_url,
            provider_id: provider_id.into(),
            title: None,
            format,
            output_path: None,
            created_at: Utc::now(),
            max_height: None,
            audio_only: false,
            referer: None,
        }
    }
}
