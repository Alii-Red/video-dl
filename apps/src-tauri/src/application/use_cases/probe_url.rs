use std::sync::Arc;

use crate::application::error::ApplicationError;
use crate::application::events::DomainEvent;
use crate::application::ports::{EventPublisher, MediaProbe};
use crate::domain::entities::MediaSource;
use crate::domain::value_objects::MediaUrl;

/// Validate a raw URL, probe it, and announce the detected media.
pub struct ProbeUrl {
    probe: Arc<dyn MediaProbe>,
    events: Arc<dyn EventPublisher>,
}

impl ProbeUrl {
    pub fn new(probe: Arc<dyn MediaProbe>, events: Arc<dyn EventPublisher>) -> Self {
        Self { probe, events }
    }

    pub async fn execute(&self, raw_url: &str) -> Result<MediaSource, ApplicationError> {
        let url = MediaUrl::parse(raw_url)?;
        let source = self.probe.probe(&url, None).await?;
        self.events.publish(DomainEvent::MediaDetected {
            source: source.clone(),
        });
        Ok(source)
    }
}
