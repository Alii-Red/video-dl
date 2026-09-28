use std::sync::Mutex;

use async_trait::async_trait;

use crate::application::error::ApplicationError;
use crate::application::ports::SettingsRepository;
use crate::domain::value_objects::Settings;

/// In-memory settings store for Phase 1 defaults and tests.
#[derive(Default)]
pub struct InMemorySettingsRepository {
    settings: Mutex<Settings>,
}

impl InMemorySettingsRepository {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl SettingsRepository for InMemorySettingsRepository {
    async fn load(&self) -> Result<Settings, ApplicationError> {
        Ok(self.settings.lock().unwrap().clone())
    }

    async fn save(&self, settings: &Settings) -> Result<(), ApplicationError> {
        *self.settings.lock().unwrap() = settings.clone();
        Ok(())
    }
}
