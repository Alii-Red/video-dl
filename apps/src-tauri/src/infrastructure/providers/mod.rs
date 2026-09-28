//! Providers (Phase 7): per-site detection + yt-dlp arg tailoring.
//!
//! Providers are compiled statically and registered at runtime in a
//! [`ProviderRegistry`] (no dynamic libraries — see ADR Partie 6/12). The core
//! never depends on a concrete provider: the scheduler talks to the
//! [`crate::application::ports::DownloadPlanner`] port, implemented here by
//! [`RegistryPlanner`].

mod generic;
mod reddit;
mod vimeo;
mod youtube;

use std::sync::Arc;

use async_trait::async_trait;

use crate::application::error::ApplicationError;
use crate::application::ports::{DownloadPlanner, EngineRequest};
use crate::domain::entities::Download;

pub use generic::GenericProvider;
pub use reddit::RedditProvider;
pub use vimeo::VimeoProvider;
pub use youtube::YouTubeProvider;

/// What a provider declares it can do (informational; drives UI affordances).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Capabilities {
    pub playlist: bool,
    pub live: bool,
    pub subtitles: bool,
    pub chapters: bool,
}

/// User-chosen download options that shape yt-dlp format selection.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DownloadOptions {
    pub max_height: Option<u32>,
    pub audio_only: bool,
    /// Fetch and embed subtitles when the site provides them.
    pub subtitles: bool,
}

pub trait Provider: Send + Sync {
    fn id(&self) -> &'static str;
    fn matches(&self, url: &str) -> bool;
    fn capabilities(&self) -> Capabilities;
    /// Provider-specific yt-dlp args (format selection, extras). The engine adds
    /// the common flags (progress template, output dir, etc.).
    fn build_ytdlp_args(&self, url: &str, options: &DownloadOptions) -> Vec<String>;
}

/// Shared yt-dlp `-f` selection (plus optional subtitle flags) used by most
/// providers. `height<=?N` is yt-dlp's "soft" comparison: it won't exclude
/// formats that lack a height.
pub fn ytdlp_format_args(options: &DownloadOptions) -> Vec<String> {
    let mut args = if options.audio_only {
        vec!["-f".into(), "bestaudio/best".into()]
    } else {
        match options.max_height {
            Some(h) => vec![
                "-f".into(),
                format!("bv*[height<=?{h}]+ba/b[height<=?{h}]/b"),
            ],
            None => vec!["-f".into(), "bv*+ba/b".into()],
        }
    };
    if options.subtitles && !options.audio_only {
        // Fetch available subs and embed them into the container. yt-dlp is a
        // no-op for sites without subtitles.
        args.push("--write-subs".into());
        args.push("--embed-subs".into());
        args.push("--sub-langs".into());
        args.push("all".into());
    }
    args
}

/// Holds the registered providers and a generic fallback.
pub struct ProviderRegistry {
    providers: Vec<Arc<dyn Provider>>,
    fallback: Arc<dyn Provider>,
}

impl ProviderRegistry {
    /// The providers in scope: youtube, reddit, vimeo, with generic as fallback.
    pub fn with_defaults() -> Self {
        Self {
            providers: vec![
                Arc::new(YouTubeProvider),
                Arc::new(RedditProvider),
                Arc::new(VimeoProvider),
            ],
            fallback: Arc::new(GenericProvider),
        }
    }

    /// Register an extra provider (checked before the generic fallback).
    pub fn register(&mut self, provider: Arc<dyn Provider>) {
        self.providers.push(provider);
    }

    /// First provider whose `matches` returns true, else the generic fallback.
    pub fn resolve(&self, url: &str) -> Arc<dyn Provider> {
        self.providers
            .iter()
            .find(|p| p.matches(url))
            .cloned()
            .unwrap_or_else(|| self.fallback.clone())
    }
}

impl Default for ProviderRegistry {
    fn default() -> Self {
        Self::with_defaults()
    }
}

/// Adapts the registry to the `DownloadPlanner` port the scheduler depends on.
pub struct RegistryPlanner {
    registry: ProviderRegistry,
    output_dir: String,
    options: DownloadOptions,
}

impl RegistryPlanner {
    pub fn new(
        registry: ProviderRegistry,
        output_dir: impl Into<String>,
        options: DownloadOptions,
    ) -> Self {
        Self {
            registry,
            output_dir: output_dir.into(),
            options,
        }
    }
}

#[async_trait]
impl DownloadPlanner for RegistryPlanner {
    async fn plan(&self, download: &Download) -> Result<EngineRequest, ApplicationError> {
        let url = download.source_url.as_str();
        let provider = self.registry.resolve(url);
        // Per-download quality (chosen in the widget) overrides the global
        // default; subtitles stay a global preference.
        let options = DownloadOptions {
            max_height: download.max_height.or(self.options.max_height),
            audio_only: download.audio_only || self.options.audio_only,
            subtitles: self.options.subtitles,
        };
        // A plain progressive file (mp4/webm/mov, not an HLS/DASH manifest)
        // that no site-specific provider claimed goes straight through HTTP
        // instead of yt-dlp's generic extractor. That extractor applies
        // heuristics meant for arbitrary web pages — it can refuse a link
        // we've already positively identified as a real media file (e.g. a
        // KVS/tube player's direct URL, which redirects through a path yt-dlp
        // flags as unsafe purely on its extension). We already know exactly
        // what this is, so there's nothing left to extract.
        let is_direct_file = provider.id() == "generic" && !download.format.is_manifest();

        Ok(EngineRequest {
            url: url.to_string(),
            output_dir: self.output_dir.clone(),
            ytdlp_args: provider.build_ytdlp_args(url, &options),
            title: download.title.clone(),
            direct_url: is_direct_file.then(|| url.to_string()),
            referer: download.referer.clone(),
        })
    }
}
