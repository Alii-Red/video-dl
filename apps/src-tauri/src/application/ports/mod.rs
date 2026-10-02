//! Ports — the interfaces (traits) that infrastructure implements. The dependency
//! inversion lives here: application defines the contracts, infrastructure fulfils
//! them, and use cases depend only on these traits (via `Arc<dyn Trait>`).

mod download_engine;
mod download_planner;
mod event_publisher;
mod media_probe;
mod page_extractor;
mod playlist_resolver;
mod repositories;
mod settings_repository;
mod task_controller;

pub use download_engine::{DownloadEngine, EngineOutcome, EngineRequest};
pub use download_planner::DownloadPlanner;
pub use event_publisher::EventPublisher;
pub use media_probe::MediaProbe;
pub use page_extractor::{ExtractedCandidate, PageMediaExtractor};
pub use playlist_resolver::{PlaylistEntry, PlaylistResolver, ResolvedPlaylist};
pub use repositories::DownloadRepository;
pub use settings_repository::SettingsRepository;
pub use task_controller::TaskController;
