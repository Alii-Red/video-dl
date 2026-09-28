//! Provider registry + planner tests.

use video_dl::application::ports::DownloadPlanner;
use video_dl::domain::entities::Download;
use video_dl::domain::value_objects::{MediaFormat, MediaUrl};
use video_dl::infrastructure::providers::{
    ytdlp_format_args, DownloadOptions, ProviderRegistry, RegistryPlanner,
};

#[test]
fn registry_resolves_known_sites() {
    let reg = ProviderRegistry::with_defaults();
    assert_eq!(reg.resolve("https://www.youtube.com/watch?v=x").id(), "youtube");
    assert_eq!(reg.resolve("https://youtu.be/x").id(), "youtube");
    assert_eq!(reg.resolve("https://www.reddit.com/r/x/y").id(), "reddit");
    assert_eq!(reg.resolve("https://v.redd.it/abc").id(), "reddit");
    assert_eq!(reg.resolve("https://vimeo.com/12345").id(), "vimeo");
}

#[test]
fn registry_falls_back_to_generic() {
    let reg = ProviderRegistry::with_defaults();
    assert_eq!(reg.resolve("https://example.com/video.mp4").id(), "generic");
}

#[test]
fn format_args_respect_options() {
    let default = ytdlp_format_args(&DownloadOptions::default());
    assert_eq!(default, vec!["-f", "bv*+ba/b"]);

    let capped = ytdlp_format_args(&DownloadOptions {
        max_height: Some(720),
        ..Default::default()
    });
    assert!(capped[1].contains("height<=?720"));

    let audio = ytdlp_format_args(&DownloadOptions {
        audio_only: true,
        ..Default::default()
    });
    assert_eq!(audio, vec!["-f", "bestaudio/best"]);
}

#[test]
fn subtitles_add_flags_except_in_audio_mode() {
    let with_subs = ytdlp_format_args(&DownloadOptions {
        subtitles: true,
        ..Default::default()
    });
    assert!(with_subs.contains(&"--embed-subs".to_string()));

    // Audio-only downloads skip subtitle flags.
    let audio_subs = ytdlp_format_args(&DownloadOptions {
        subtitles: true,
        audio_only: true,
        ..Default::default()
    });
    assert!(!audio_subs.contains(&"--embed-subs".to_string()));
}

#[test]
fn youtube_provider_embeds_chapters() {
    let reg = ProviderRegistry::with_defaults();
    let args = reg
        .resolve("https://youtube.com/watch?v=x")
        .build_ytdlp_args("https://youtube.com/watch?v=x", &DownloadOptions::default());
    assert!(args.contains(&"--embed-chapters".to_string()));
}

#[tokio::test]
async fn planner_builds_engine_request() {
    let planner = RegistryPlanner::new(
        ProviderRegistry::with_defaults(),
        "C:/downloads",
        DownloadOptions::default(),
    );
    let url = MediaUrl::parse("https://youtu.be/abc").unwrap();
    let download = Download::new(url, "youtube", MediaFormat::ProgressiveMp4);

    let req = planner.plan(&download).await.unwrap();
    assert_eq!(req.url, "https://youtu.be/abc");
    assert_eq!(req.output_dir, "C:/downloads");
    assert_eq!(req.ytdlp_args[0], "-f");
    assert!(req.ytdlp_args.contains(&"--embed-chapters".to_string()));
}

#[tokio::test]
async fn planner_applies_per_download_quality_over_global_default() {
    // Global default = best; this download overrides to 720p.
    let planner = RegistryPlanner::new(
        ProviderRegistry::with_defaults(),
        "C:/downloads",
        DownloadOptions::default(),
    );
    let url = MediaUrl::parse("https://youtu.be/abc").unwrap();
    let mut download = Download::new(url, "youtube", MediaFormat::ProgressiveMp4);
    download.max_height = Some(720);

    let req = planner.plan(&download).await.unwrap();
    let selector = &req.ytdlp_args[1];
    assert!(
        selector.contains("height<=?720"),
        "expected a 720p cap in the format selector, got: {selector}"
    );
}

#[tokio::test]
async fn planner_applies_per_download_audio_only() {
    let planner = RegistryPlanner::new(
        ProviderRegistry::with_defaults(),
        "C:/downloads",
        DownloadOptions::default(),
    );
    let url = MediaUrl::parse("https://youtu.be/abc").unwrap();
    let mut download = Download::new(url, "youtube", MediaFormat::ProgressiveMp4);
    download.audio_only = true;

    let req = planner.plan(&download).await.unwrap();
    assert_eq!(req.ytdlp_args[1], "bestaudio/best");
}

#[tokio::test]
async fn planner_routes_unrecognized_progressive_file_direct_over_http() {
    // No provider claims this host — generic fallback — and it's a plain
    // progressive file, not a manifest: should bypass yt-dlp entirely (see
    // ADR follow-up: yt-dlp's generic extractor refuses some real direct
    // links, e.g. KVS/tube players redirecting through an unusual path).
    let planner = RegistryPlanner::new(
        ProviderRegistry::with_defaults(),
        "C:/downloads",
        DownloadOptions::default(),
    );
    let url = MediaUrl::parse("https://pimpbunny.com/get_file/45/abc/1/2/586179_720p.mp4/").unwrap();
    let mut download = Download::new(url, "generic", MediaFormat::ProgressiveMp4);
    download.referer = Some("https://pimpbunny.com/videos/adria-rae/".into());

    let req = planner.plan(&download).await.unwrap();
    assert_eq!(
        req.direct_url.as_deref(),
        Some("https://pimpbunny.com/get_file/45/abc/1/2/586179_720p.mp4/")
    );
    assert_eq!(req.referer.as_deref(), Some("https://pimpbunny.com/videos/adria-rae/"));
}

#[tokio::test]
async fn planner_does_not_route_hls_manifests_direct_even_if_unrecognized() {
    // A manifest still needs yt-dlp to fetch/mux the actual segments — only
    // plain progressive files get the direct-HTTP shortcut.
    let planner = RegistryPlanner::new(
        ProviderRegistry::with_defaults(),
        "C:/downloads",
        DownloadOptions::default(),
    );
    let url = MediaUrl::parse("https://cdn.example.com/stream/master.m3u8").unwrap();
    let download = Download::new(url, "generic", MediaFormat::Hls);

    let req = planner.plan(&download).await.unwrap();
    assert_eq!(req.direct_url, None);
}

#[tokio::test]
async fn planner_does_not_route_provider_specific_sites_direct() {
    // YouTube resolves to a specific provider — always goes through yt-dlp,
    // even for a (hypothetical) progressive-looking URL.
    let planner = RegistryPlanner::new(
        ProviderRegistry::with_defaults(),
        "C:/downloads",
        DownloadOptions::default(),
    );
    let url = MediaUrl::parse("https://youtu.be/abc").unwrap();
    let download = Download::new(url, "youtube", MediaFormat::ProgressiveMp4);

    let req = planner.plan(&download).await.unwrap();
    assert_eq!(req.direct_url, None);
}
