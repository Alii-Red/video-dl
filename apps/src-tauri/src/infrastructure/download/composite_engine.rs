//! Routes each download to the right engine: a direct HTTP stream when the
//! planner marked the request as one (`direct_url` set — a plain file yt-dlp
//! would mishandle), yt-dlp otherwise. Keeps `TaskScheduler` engine-agnostic
//! (it only knows the single `DownloadEngine` port).

use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::application::error::ApplicationError;
use crate::application::ports::{DownloadEngine, EngineOutcome, EngineRequest};
use crate::domain::value_objects::Progress;

use super::{HttpDownloadEngine, YtDlpEngine};

pub struct CompositeEngine {
    ytdlp: YtDlpEngine,
    http: HttpDownloadEngine,
}

impl CompositeEngine {
    pub fn new(ytdlp: YtDlpEngine, http: HttpDownloadEngine) -> Self {
        Self { ytdlp, http }
    }
}

#[async_trait]
impl DownloadEngine for CompositeEngine {
    async fn run(
        &self,
        request: EngineRequest,
        progress: mpsc::Sender<Progress>,
    ) -> Result<EngineOutcome, ApplicationError> {
        if request.direct_url.is_some() {
            self.http.run(request, progress).await
        } else {
            self.ytdlp.run(request, progress).await
        }
    }
}
