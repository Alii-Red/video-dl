//! Infrastructure layer — concrete implementations of the application ports.
//!
//! Phase 1 ships in-memory implementations so the core compiles and is testable
//! without SQLite or the network. Later phases add modules here:
//!   - `database`  → SQLx repository (Phase 3)
//!   - `download`  → yt-dlp engine + FFmpeg (Phase 4)
//!   - `network`   → probe, HLS/DASH parsing, JS injection, MITM proxy (Phase 6/9)
//!   - `providers` → generic/youtube/reddit/vimeo (Phase 7)
//!   - `scheduler` → task queue + concurrency (Phase 5)

pub mod database;
pub mod download;
pub mod events;
pub mod network;
pub mod providers;
pub mod scheduler;
