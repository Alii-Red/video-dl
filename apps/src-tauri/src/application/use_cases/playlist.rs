use std::sync::Arc;

use uuid::Uuid;

use crate::application::error::ApplicationError;
use crate::application::events::DomainEvent;
use crate::application::ports::{
    DownloadRepository, EventPublisher, PlaylistResolver, ResolvedPlaylist,
};
use crate::domain::entities::{Download, Playlist, Task};
use crate::domain::value_objects::{MediaFormat, MediaUrl};

/// Expand a URL into its playlist entries, if it is one.
pub struct ResolvePlaylist {
    resolver: Arc<dyn PlaylistResolver>,
}

impl ResolvePlaylist {
    pub fn new(resolver: Arc<dyn PlaylistResolver>) -> Self {
        Self { resolver }
    }

    pub async fn execute(&self, raw_url: &str) -> Result<Option<ResolvedPlaylist>, ApplicationError> {
        let url = MediaUrl::parse(raw_url)?;
        self.resolver.resolve(url.as_str()).await
    }
}

/// One video the user kept from a resolved playlist.
pub struct PlaylistItemRequest {
    pub url: String,
    pub title: Option<String>,
}

pub struct StartPlaylistRequest {
    pub source_url: String,
    pub title: String,
    pub items: Vec<PlaylistItemRequest>,
    pub max_height: Option<u32>,
    pub audio_only: bool,
}

pub struct StartPlaylistResult {
    pub playlist_id: Uuid,
    /// `(download_id, task_id)` per item, in playlist order.
    pub downloads: Vec<(Uuid, Uuid)>,
}

/// Queue every selected entry of a playlist as its own download, grouped
/// under one persisted [`Playlist`]. Each entry is a single-video page URL, so
/// the planner/engine handle it exactly like a manually pasted video.
pub struct StartPlaylistDownload {
    repo: Arc<dyn DownloadRepository>,
    events: Arc<dyn EventPublisher>,
}

impl StartPlaylistDownload {
    pub fn new(repo: Arc<dyn DownloadRepository>, events: Arc<dyn EventPublisher>) -> Self {
        Self { repo, events }
    }

    pub async fn execute(
        &self,
        req: StartPlaylistRequest,
    ) -> Result<StartPlaylistResult, ApplicationError> {
        let source_url = MediaUrl::parse(&req.source_url)?;
        if req.items.is_empty() {
            return Err(ApplicationError::NotFound("playlist has no items".into()));
        }

        let mut playlist = Playlist::new(req.title, source_url);
        let mut rows = Vec::with_capacity(req.items.len());
        for item in req.items {
            let url = MediaUrl::parse(&item.url)?;
            // The provider is re-resolved by host at plan time; the format is
            // informational for page URLs (yt-dlp muxes to mp4).
            let mut download = Download::new(url, "auto", MediaFormat::ProgressiveMp4);
            download.title = item.title;
            download.max_height = req.max_height;
            download.audio_only = req.audio_only;
            playlist.push(download.id);
            let task = Task::new(download.id);
            rows.push((download, task));
        }

        let ids: Vec<(Uuid, Uuid)> = rows.iter().map(|(d, t)| (d.id, t.id)).collect();
        let playlist_id = playlist.id;
        self.repo.create_playlist(playlist, rows).await?;
        for &(download_id, task_id) in &ids {
            self.events.publish(DomainEvent::DownloadCreated {
                download_id,
                task_id,
            });
        }

        Ok(StartPlaylistResult {
            playlist_id,
            downloads: ids,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::ports::PlaylistEntry;
    use crate::domain::entities::TaskState;
    use crate::infrastructure::database::InMemoryDownloadRepository;
    use crate::infrastructure::events::InMemoryEventBus;
    use async_trait::async_trait;

    struct FakeResolver;

    #[async_trait]
    impl PlaylistResolver for FakeResolver {
        async fn resolve(&self, url: &str) -> Result<Option<ResolvedPlaylist>, ApplicationError> {
            Ok(url.contains("list=").then(|| ResolvedPlaylist {
                title: "L".into(),
                entries: vec![PlaylistEntry {
                    url: "https://www.youtube.com/watch?v=a".into(),
                    title: Some("A".into()),
                    duration_secs: None,
                    thumbnail: None,
                }],
            }))
        }
    }

    #[tokio::test]
    async fn resolve_rejects_invalid_url_and_passes_through() {
        let uc = ResolvePlaylist::new(Arc::new(FakeResolver));
        assert!(uc.execute("not a url").await.is_err());
        assert!(uc
            .execute("https://www.youtube.com/watch?v=a")
            .await
            .unwrap()
            .is_none());
        let pl = uc
            .execute("https://www.youtube.com/playlist?list=PL1")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(pl.entries.len(), 1);
    }

    #[tokio::test]
    async fn queues_one_download_per_item_with_shared_options() {
        let repo = Arc::new(InMemoryDownloadRepository::new());
        let bus = Arc::new(InMemoryEventBus::new(16));
        let mut rx = bus.subscribe();
        let uc = StartPlaylistDownload::new(repo.clone(), bus);

        let res = uc
            .execute(StartPlaylistRequest {
                source_url: "https://www.youtube.com/playlist?list=PL1".into(),
                title: "My list".into(),
                items: vec![
                    PlaylistItemRequest {
                        url: "https://www.youtube.com/watch?v=a".into(),
                        title: Some("A".into()),
                    },
                    PlaylistItemRequest {
                        url: "https://www.youtube.com/watch?v=b".into(),
                        title: None,
                    },
                ],
                max_height: Some(720),
                audio_only: false,
            })
            .await
            .unwrap();

        assert_eq!(res.downloads.len(), 2);
        let queued = repo.list_tasks_by_state(TaskState::Queued).await.unwrap();
        assert_eq!(queued.len(), 2);
        let (first_dl, _) = res.downloads[0];
        let dl = repo.get_download(first_dl).await.unwrap();
        assert_eq!(dl.title.as_deref(), Some("A"));
        assert_eq!(dl.max_height, Some(720));
        for _ in 0..2 {
            assert!(matches!(
                rx.recv().await.unwrap(),
                DomainEvent::DownloadCreated { .. }
            ));
        }
    }

    #[tokio::test]
    async fn empty_selection_is_rejected() {
        let repo = Arc::new(InMemoryDownloadRepository::new());
        let uc = StartPlaylistDownload::new(repo, Arc::new(InMemoryEventBus::new(4)));
        let res = uc
            .execute(StartPlaylistRequest {
                source_url: "https://www.youtube.com/playlist?list=PL1".into(),
                title: "Empty".into(),
                items: vec![],
                max_height: None,
                audio_only: false,
            })
            .await;
        assert!(res.is_err());
    }
}
