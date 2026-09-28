use async_trait::async_trait;

use crate::application::error::ApplicationError;
use crate::application::ports::MediaProbe;
use crate::domain::entities::MediaSource;
use crate::domain::value_objects::{MediaFormat, MediaUrl};

use super::classify::classify;
use super::dash::analyze_dash;
use super::hls::analyze_hls;

/// Real detection probe: fetches the URL, classifies it by `Content-Type` /
/// extension, and — for HLS/DASH manifests — parses the body to extract
/// qualities and detect protection. This is the "manual import" path of the
/// ADR's Partie 4 (proxy MITM and JS injection feed the same pipeline later).
pub struct HttpMediaProbe {
    client: reqwest::Client,
}

impl HttpMediaProbe {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .user_agent(concat!("VideoDL/", env!("CARGO_PKG_VERSION")))
            .build()
            .expect("failed to build reqwest client");
        Self { client }
    }
}

impl Default for HttpMediaProbe {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl MediaProbe for HttpMediaProbe {
    async fn probe(
        &self,
        url: &MediaUrl,
        referer: Option<&str>,
    ) -> Result<MediaSource, ApplicationError> {
        let mut req = self.client.get(url.as_str());
        if let Some(referer) = referer {
            req = req.header(reqwest::header::REFERER, referer);
            // Some CDNs check Origin instead of/in addition to Referer;
            // derive it (scheme + host) rather than require a second field
            // from every caller.
            if let Ok(parsed) = reqwest::Url::parse(referer) {
                req = req.header(reqwest::header::ORIGIN, parsed.origin().ascii_serialization());
            }
        }
        let response = req
            .send()
            .await
            .map_err(|e| ApplicationError::Probe(format!("request failed: {e}")))?;

        if !response.status().is_success() {
            return Err(ApplicationError::Probe(format!(
                "server returned {}",
                response.status()
            )));
        }

        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);

        let format = classify(url.as_str(), content_type.as_deref())
            .ok_or_else(|| ApplicationError::Probe("unrecognized media type".into()))?;

        match format {
            MediaFormat::Hls => {
                let body = read_body(response).await?;
                let info = analyze_hls(&body)?;
                Ok(build_source(url, format, info.qualities, info.protected))
            }
            MediaFormat::Dash => {
                let body = read_body(response).await?;
                let info = analyze_dash(&body)?;
                Ok(build_source(url, format, info.qualities, info.protected))
            }
            progressive => {
                // Headers are enough; don't download the file body here.
                drop(response);
                Ok(MediaSource::new(url.clone(), progressive))
            }
        }
    }
}

async fn read_body(response: reqwest::Response) -> Result<String, ApplicationError> {
    response
        .text()
        .await
        .map_err(|e| ApplicationError::Probe(format!("reading body: {e}")))
}

fn build_source(
    url: &MediaUrl,
    format: MediaFormat,
    qualities: Vec<crate::domain::value_objects::Quality>,
    protected: bool,
) -> MediaSource {
    let mut source = MediaSource::new(url.clone(), format).with_qualities(qualities);
    source.protected = protected;
    source
}
