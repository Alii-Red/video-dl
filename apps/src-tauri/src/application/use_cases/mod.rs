//! Use cases — one intention per file. Each holds its dependencies as
//! `Arc<dyn Port>` and exposes an `execute`-style method.

mod cancel_download;
mod delete_history_entry;
mod extract_from_page;
mod ingest_detection;
mod list_active;
mod list_history;
mod pause_resume;
mod probe_url;
mod recover_interrupted;
mod remove_download;
mod retry_download;
mod settings;
mod start_download;

pub use cancel_download::CancelDownload;
pub use delete_history_entry::DeleteHistoryEntry;
pub use extract_from_page::ExtractFromPage;
pub use ingest_detection::{DetectionMeta, IngestDetection};
pub use list_active::ListActiveDownloads;
pub use list_history::ListHistory;
pub use pause_resume::PauseResume;
pub use probe_url::ProbeUrl;
pub use recover_interrupted::RecoverInterruptedTasks;
pub use remove_download::RemoveDownload;
pub use retry_download::RetryDownload;
pub use settings::{GetSettings, UpdateSettings};
pub use start_download::{StartDownload, StartDownloadRequest, StartDownloadResult};
