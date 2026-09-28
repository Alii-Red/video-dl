use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// An archived completed download (read model over the `history` table joined
/// with `downloads` for display fields).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub download_id: Uuid,
    pub title: Option<String>,
    pub source_url: String,
    pub final_path: Option<String>,
    pub bytes: Option<u64>,
    pub completed_at: DateTime<Utc>,
}
