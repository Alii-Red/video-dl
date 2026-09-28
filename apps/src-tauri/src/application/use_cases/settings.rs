use std::sync::Arc;

use crate::application::error::ApplicationError;
use crate::application::ports::SettingsRepository;
use crate::domain::value_objects::Settings;

pub struct GetSettings {
    repo: Arc<dyn SettingsRepository>,
}

impl GetSettings {
    pub fn new(repo: Arc<dyn SettingsRepository>) -> Self {
        Self { repo }
    }

    pub async fn execute(&self) -> Result<Settings, ApplicationError> {
        self.repo.load().await
    }
}

pub struct UpdateSettings {
    repo: Arc<dyn SettingsRepository>,
}

impl UpdateSettings {
    pub fn new(repo: Arc<dyn SettingsRepository>) -> Self {
        Self { repo }
    }

    pub async fn execute(&self, settings: Settings) -> Result<(), ApplicationError> {
        self.repo.save(&settings).await
    }
}
