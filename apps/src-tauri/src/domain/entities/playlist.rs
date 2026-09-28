use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::value_objects::MediaUrl;

/// An ordered collection of downloads originating from one source URL.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Playlist {
    pub id: Uuid,
    pub title: String,
    pub source_url: MediaUrl,
    pub items: Vec<PlaylistItem>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaylistItem {
    pub download_id: Uuid,
    pub position: u32,
}

impl Playlist {
    pub fn new(title: impl Into<String>, source_url: MediaUrl) -> Self {
        Self {
            id: Uuid::new_v4(),
            title: title.into(),
            source_url,
            items: Vec::new(),
            created_at: Utc::now(),
        }
    }

    pub fn push(&mut self, download_id: Uuid) {
        let position = self.items.len() as u32;
        self.items.push(PlaylistItem {
            download_id,
            position,
        });
    }
}
