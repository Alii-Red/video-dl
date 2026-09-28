use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::application::ports::TaskController;

/// Registry of currently-running tasks and their cancellation tokens. Shared
/// (cheaply cloneable) between the scheduler and the cancel/pause use cases, so
/// interrupting a task drops its engine future and kills the yt-dlp child.
#[derive(Clone, Default)]
pub struct RunningTasks {
    inner: Arc<Mutex<HashMap<Uuid, CancellationToken>>>,
}

impl RunningTasks {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a task as running and return its cancellation token.
    pub fn register(&self, task_id: Uuid) -> CancellationToken {
        let token = CancellationToken::new();
        self.inner.lock().unwrap().insert(task_id, token.clone());
        token
    }

    /// Remove a task once it has finished (or been interrupted).
    pub fn finish(&self, task_id: Uuid) {
        self.inner.lock().unwrap().remove(&task_id);
    }

    /// Whether a task is currently registered as running.
    pub fn is_running(&self, task_id: Uuid) -> bool {
        self.inner.lock().unwrap().contains_key(&task_id)
    }
}

impl TaskController for RunningTasks {
    fn interrupt(&self, task_id: Uuid) {
        if let Some(token) = self.inner.lock().unwrap().get(&task_id) {
            token.cancel();
        }
    }
}
