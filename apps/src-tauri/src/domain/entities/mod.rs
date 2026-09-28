//! Entities — objects with identity and a lifecycle.

mod download;
mod history;
mod media_source;
mod playlist;
mod task;

pub use download::Download;
pub use history::HistoryEntry;
pub use media_source::MediaSource;
pub use playlist::{Playlist, PlaylistItem};
pub use task::{Task, TaskState};
