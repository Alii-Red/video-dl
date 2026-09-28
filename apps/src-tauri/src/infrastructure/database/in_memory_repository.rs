use std::collections::HashMap;
use std::sync::Mutex;

use async_trait::async_trait;
use chrono::Utc;
use uuid::Uuid;

use crate::application::error::ApplicationError;
use crate::application::ports::DownloadRepository;
use crate::domain::entities::{Download, HistoryEntry, Task, TaskState};

/// In-memory repository for Phase 1 and tests. Replaced by a SQLx implementation
/// in Phase 3. Locks are never held across an `.await`, so a std `Mutex` is fine.
#[derive(Default)]
pub struct InMemoryDownloadRepository {
    downloads: Mutex<HashMap<Uuid, Download>>,
    tasks: Mutex<HashMap<Uuid, Task>>,
    history: Mutex<Vec<HistoryEntry>>,
}

impl InMemoryDownloadRepository {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl DownloadRepository for InMemoryDownloadRepository {
    async fn create(&self, download: Download, task: Task) -> Result<(), ApplicationError> {
        self.downloads.lock().unwrap().insert(download.id, download);
        self.tasks.lock().unwrap().insert(task.id, task);
        Ok(())
    }

    async fn get_download(&self, id: Uuid) -> Result<Download, ApplicationError> {
        self.downloads
            .lock()
            .unwrap()
            .get(&id)
            .cloned()
            .ok_or_else(|| ApplicationError::NotFound(format!("download {id}")))
    }

    async fn save_download(&self, download: Download) -> Result<(), ApplicationError> {
        self.downloads.lock().unwrap().insert(download.id, download);
        Ok(())
    }

    async fn get_task(&self, id: Uuid) -> Result<Task, ApplicationError> {
        self.tasks
            .lock()
            .unwrap()
            .get(&id)
            .cloned()
            .ok_or_else(|| ApplicationError::NotFound(format!("task {id}")))
    }

    async fn save_task(&self, task: Task) -> Result<(), ApplicationError> {
        self.tasks.lock().unwrap().insert(task.id, task);
        Ok(())
    }

    async fn list_tasks_by_state(
        &self,
        state: TaskState,
    ) -> Result<Vec<Task>, ApplicationError> {
        Ok(self
            .tasks
            .lock()
            .unwrap()
            .values()
            .filter(|t| t.state == state)
            .cloned()
            .collect())
    }

    async fn list_all_tasks(&self) -> Result<Vec<Task>, ApplicationError> {
        Ok(self.tasks.lock().unwrap().values().cloned().collect())
    }

    async fn list_active(&self) -> Result<Vec<(Download, Task)>, ApplicationError> {
        let tasks = self.tasks.lock().unwrap();
        let downloads = self.downloads.lock().unwrap();
        let mut out = Vec::new();
        for task in tasks.values() {
            if !task.state.is_terminal() {
                if let Some(download) = downloads.get(&task.download_id) {
                    out.push((download.clone(), task.clone()));
                }
            }
        }
        Ok(out)
    }

    async fn record_history(
        &self,
        download_id: Uuid,
        final_path: Option<String>,
        bytes: Option<u64>,
    ) -> Result<(), ApplicationError> {
        let (title, source_url) = self
            .downloads
            .lock()
            .unwrap()
            .get(&download_id)
            .map(|d| (d.title.clone(), d.source_url.as_str().to_string()))
            .unwrap_or((None, String::new()));
        self.history.lock().unwrap().push(HistoryEntry {
            download_id,
            title,
            source_url,
            final_path,
            bytes,
            completed_at: Utc::now(),
        });
        Ok(())
    }

    async fn recent_history(&self, limit: u32) -> Result<Vec<HistoryEntry>, ApplicationError> {
        let history = self.history.lock().unwrap();
        Ok(history
            .iter()
            .rev()
            .take(limit as usize)
            .cloned()
            .collect())
    }

    async fn delete_download(&self, id: Uuid) -> Result<(), ApplicationError> {
        self.downloads.lock().unwrap().remove(&id);
        self.tasks.lock().unwrap().retain(|_, t| t.download_id != id);
        Ok(())
    }

    async fn delete_history_entry(&self, download_id: Uuid) -> Result<(), ApplicationError> {
        self.history.lock().unwrap().retain(|h| h.download_id != download_id);
        Ok(())
    }
}
