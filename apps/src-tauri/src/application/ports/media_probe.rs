use async_trait::async_trait;

use crate::application::error::ApplicationError;
use crate::domain::entities::MediaSource;
use crate::domain::value_objects::MediaUrl;

/// Detection port: given a URL, resolve it to a [`MediaSource`] (HTTP probe,
/// M3U8/MPD parsing, etc.). Real implementation lands in Phase 6.
///
/// `referer`, when known (the page the URL was found on), is sent as the
/// `Referer`/`Origin` headers — many CDNs (YouTube's googlevideo, adult tube
/// sites' anti-hotlinking guards, ...) reject a request that doesn't look
/// like it came from their own page, which otherwise makes the probe fail
/// silently with no indication why.
#[async_trait]
pub trait MediaProbe: Send + Sync {
    async fn probe(
        &self,
        url: &MediaUrl,
        referer: Option<&str>,
    ) -> Result<MediaSource, ApplicationError>;
}
