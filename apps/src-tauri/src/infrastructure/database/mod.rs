//! Persistence implementations.
//!
//! Phase 1: in-memory. Phase 3 adds a SQLx/SQLite-backed repository behind the
//! same `DownloadRepository` port — no use case changes required.

mod in_memory_repository;
mod in_memory_settings;
mod sqlx_repository;
mod sqlx_settings;

pub use in_memory_repository::InMemoryDownloadRepository;
pub use in_memory_settings::InMemorySettingsRepository;
pub use sqlx_repository::SqlxDownloadRepository;
pub use sqlx_settings::SqlxSettingsRepository;
