use async_trait::async_trait;

use crate::application::error::ApplicationError;
use crate::application::ports::MediaProbe;
use crate::domain::entities::MediaSource;
use crate::domain::value_objects::MediaUrl;

/// Placeholder probe for Phase 1. Real detection (HTTP probe, M3U8/MPD parsing,
/// JS injection, MITM proxy) arrives in Phase 6. Returns an error so wiring is
/// exercised without pretending to detect anything.
#[derive(Default)]
pub struct StubMediaProbe;

#[async_trait]
impl MediaProbe for StubMediaProbe {
    async fn probe(
        &self,
        _url: &MediaUrl,
        _referer: Option<&str>,
    ) -> Result<MediaSource, ApplicationError> {
        Err(ApplicationError::Probe(
            "probe not implemented until Phase 6".into(),
        ))
    }
}
