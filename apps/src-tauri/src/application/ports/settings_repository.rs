use async_trait::async_trait;

use crate::application::error::ApplicationError;
use crate::domain::value_objects::Settings;

/// Persistence port for application settings. `load` returns defaults when none
/// have been saved yet.
#[async_trait]
pub trait SettingsRepository: Send + Sync {
    async fn load(&self) -> Result<Settings, ApplicationError>;
    async fn save(&self, settings: &Settings) -> Result<(), ApplicationError>;
}
