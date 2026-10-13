//! Desktop shell: the only module that knows about Tauri. Thin
//! `#[tauri::command]` wrappers call the Tauri-free handlers in `ipc::commands`,
//! and a background task bridges the internal event bus to Tauri `emit`.

mod extraction;

use std::sync::Arc;

use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Emitter, Manager};

/// Bring the main window back to the foreground (tray click / menu / relaunch).
fn show_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

use crate::application::use_cases::{ExtractFromPage, IngestDetection, ResolvePlaylist};
use crate::infrastructure::download::{
    CompositeEngine, HttpDownloadEngine, YtDlpEngine, YtDlpPlaylistResolver,
};
use crate::infrastructure::network::{ExtensionWsBridge, ExtensionWsHandle, HttpMediaProbe};
use crate::infrastructure::providers::{DownloadOptions, ProviderRegistry, RegistryPlanner};
use crate::infrastructure::scheduler::TaskScheduler;
use crate::ipc::commands;
use crate::ipc::dto::{
    DownloadViewDto, HistoryEntryDto, MediaSourceDto, PlaylistDto, SettingsDto,
    StartDownloadArgs, StartDownloadResponse, StartPlaylistArgs, StartPlaylistResponse,
};
use crate::ipc::error::{IpcError, IpcErrorKind};
use crate::ipc::events::IpcEvent;
use crate::state::AppState;

use extraction::{ExtractorRegistry, TauriPageExtractor};

type Cmd<T> = Result<T, IpcError>;

/// Holds the scheduler so `start_download`/`resume_download` can trigger a
/// dispatch pass without threading it through every command.
struct SchedulerHandle(TaskScheduler);

#[tauri::command]
async fn probe_url(state: tauri::State<'_, AppState>, url: String) -> Cmd<MediaSourceDto> {
    commands::probe_url(&state, url).await
}

#[tauri::command]
async fn extract_from_page(
    extractor: tauri::State<'_, ExtractFromPage>,
    url: String,
) -> Cmd<Vec<MediaSourceDto>> {
    commands::extract_from_page(&extractor, url).await
}

#[tauri::command]
async fn open_interactive_extraction(
    extractor: tauri::State<'_, ExtractFromPage>,
    url: String,
) -> Cmd<()> {
    extractor.execute_interactive(&url).await?;
    Ok(())
}

#[tauri::command]
async fn start_download(
    state: tauri::State<'_, AppState>,
    scheduler: tauri::State<'_, SchedulerHandle>,
    args: StartDownloadArgs,
) -> Cmd<StartDownloadResponse> {
    let res = commands::start_download(&state, args).await?;
    dispatch(&scheduler.0);
    Ok(res)
}

#[tauri::command]
async fn resolve_playlist(
    resolver: tauri::State<'_, ResolvePlaylist>,
    url: String,
) -> Cmd<Option<PlaylistDto>> {
    commands::resolve_playlist(&resolver, url).await
}

#[tauri::command]
async fn start_playlist_download(
    state: tauri::State<'_, AppState>,
    scheduler: tauri::State<'_, SchedulerHandle>,
    args: StartPlaylistArgs,
) -> Cmd<StartPlaylistResponse> {
    let res = commands::start_playlist_download(&state, args).await?;
    dispatch(&scheduler.0);
    Ok(res)
}

#[tauri::command]
async fn cancel_download(state: tauri::State<'_, AppState>, task_id: String) -> Cmd<()> {
    commands::cancel_download(&state, task_id).await
}

#[tauri::command]
async fn pause_download(state: tauri::State<'_, AppState>, task_id: String) -> Cmd<()> {
    commands::pause_download(&state, task_id).await
}

#[tauri::command]
async fn resume_download(
    state: tauri::State<'_, AppState>,
    scheduler: tauri::State<'_, SchedulerHandle>,
    task_id: String,
) -> Cmd<()> {
    commands::resume_download(&state, task_id).await?;
    dispatch(&scheduler.0);
    Ok(())
}

#[tauri::command]
async fn list_active(state: tauri::State<'_, AppState>) -> Cmd<Vec<DownloadViewDto>> {
    commands::list_active(&state).await
}

#[tauri::command]
async fn list_history(state: tauri::State<'_, AppState>) -> Cmd<Vec<HistoryEntryDto>> {
    commands::list_history(&state).await
}

/// Dismiss a failed/cancelled entry from the queue view for good — unlike
/// clearing it from the frontend store, this deletes the underlying
/// download/task rows so it doesn't reappear on the next reload.
#[tauri::command]
async fn remove_download(state: tauri::State<'_, AppState>, task_id: String) -> Cmd<()> {
    commands::remove_download(&state, task_id).await
}

/// Re-queue a failed download and kick the scheduler (handy for YouTube's
/// intermittent 403s — a fresh run usually succeeds).
#[tauri::command]
async fn retry_download(
    state: tauri::State<'_, AppState>,
    scheduler: tauri::State<'_, SchedulerHandle>,
    task_id: String,
) -> Cmd<()> {
    commands::retry_download(&state, task_id).await?;
    dispatch(&scheduler.0);
    Ok(())
}

#[tauri::command]
async fn delete_history_entry(state: tauri::State<'_, AppState>, download_id: String) -> Cmd<()> {
    commands::delete_history_entry(&state, download_id).await
}

/// Widget "refresh" button: ask the connected extension to re-scan the active
/// tab now (picks up a video the page loaded after the last scan, without the
/// user reloading the page). No-op if no extension is connected.
#[tauri::command]
fn rescan_extension(handle: tauri::State<'_, ExtensionWsHandle>) {
    handle.request_rescan();
}

/// Reveal a downloaded file in Explorer (select it, not just open its
/// folder). Windows-only shortcut — `explorer.exe` handles a missing file
/// gracefully by just opening the parent folder.
#[tauri::command]
fn open_containing_folder(path: String) -> Cmd<()> {
    open_in_explorer(&path)
}

/// Opens Explorer with the file selected (Windows). Shared by the history
/// "open folder" action and the widget's post-download "open folder" button.
fn open_in_explorer(path: &str) -> Cmd<()> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt as _;
        const NO_WINDOW: u32 = 0x0800_0000;
        std::process::Command::new("explorer.exe")
            .arg("/select,")
            .raw_arg(path)
            .creation_flags(NO_WINDOW)
            .spawn()
            .map_err(|e| IpcError::new(IpcErrorKind::Engine, format!("explorer: {e}")))?;
    }
    #[cfg(not(windows))]
    {
        let _ = path;
    }
    Ok(())
}

/// Managed flag: whether the FFmpeg the engine resolved is a *full* build (can
/// remux HLS audio) rather than the bundled minimal one — drives a Settings
/// warning banner so a broken HLS remux isn't a silent mystery.
struct FfmpegFull(bool);

/// Managed path to the bundled runtime binaries dir (resolved once at startup),
/// so commands like `update_ytdlp` can find the yt-dlp executable.
struct BinariesDir(std::path::PathBuf);

/// Managed path to the bundled browser-extension folder (so the "set up
/// extension" button can reveal it in Explorer for a one-click "load unpacked").
struct ExtensionDir(std::path::PathBuf);

/// Open an http(s) URL in the user's default browser (e.g. the "go to
/// anime-sama" shortcut). Restricted to web URLs.
#[tauri::command]
fn open_external(url: String) -> Cmd<()> {
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return Err(IpcError::new(
            IpcErrorKind::Validation,
            "URL non http(s)".to_string(),
        ));
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt as _;
        const NO_WINDOW: u32 = 0x0800_0000;
        // explorer.exe opens an http URL in the default browser, and takes the
        // URL as a single argument (no shell quoting pitfalls).
        std::process::Command::new("explorer.exe")
            .arg(&url)
            .creation_flags(NO_WINDOW)
            .spawn()
            .map_err(|e| IpcError::new(IpcErrorKind::Engine, format!("explorer: {e}")))?;
    }
    #[cfg(not(windows))]
    {
        let _ = url;
    }
    Ok(())
}

/// Open the bundled extension folder in Explorer so the user can point the
/// browser's "load unpacked" at it. (The token is auto-configured separately via
/// the app's /config endpoint, so there's nothing to copy by hand.)
#[tauri::command]
fn reveal_extension_folder(dir: tauri::State<'_, ExtensionDir>) -> Cmd<()> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt as _;
        const NO_WINDOW: u32 = 0x0800_0000;
        std::process::Command::new("explorer.exe")
            .raw_arg(format!("\"{}\"", dir.0.display()))
            .creation_flags(NO_WINDOW)
            .spawn()
            .map_err(|e| IpcError::new(IpcErrorKind::Engine, format!("explorer: {e}")))?;
    }
    #[cfg(not(windows))]
    {
        let _ = &dir;
    }
    Ok(())
}

#[tauri::command]
fn ffmpeg_is_full(status: tauri::State<'_, FfmpegFull>) -> bool {
    status.0
}

/// Whether the Opera GX extension is currently connected to the WS bridge —
/// drives the widget's live connection dot.
#[tauri::command]
fn extension_connected(handle: tauri::State<'_, ExtensionWsHandle>) -> bool {
    handle.is_connected()
}

/// Open the folder of a finished download (widget "open folder" button). The
/// row may still be live or already archived to history — try both.
#[tauri::command]
async fn open_download_folder(
    state: tauri::State<'_, AppState>,
    download_id: String,
) -> Cmd<()> {
    let id = uuid::Uuid::parse_str(&download_id)
        .map_err(|e| IpcError::new(IpcErrorKind::Validation, format!("id invalide: {e}")))?;
    let mut path = state
        .download_repo
        .get_download(id)
        .await
        .ok()
        .and_then(|d| d.output_path);
    if path.is_none() {
        if let Ok(history) = state.download_repo.recent_history(200).await {
            path = history
                .into_iter()
                .find(|h| h.download_id == id)
                .and_then(|h| h.final_path);
        }
    }
    let path = path
        .ok_or_else(|| IpcError::new(IpcErrorKind::Engine, "fichier introuvable".to_string()))?;
    open_in_explorer(&path)
}

#[tauri::command]
async fn get_settings(state: tauri::State<'_, AppState>) -> Cmd<SettingsDto> {
    commands::get_settings(&state).await
}

#[tauri::command]
async fn update_settings(state: tauri::State<'_, AppState>, settings: SettingsDto) -> Cmd<()> {
    commands::update_settings(&state, settings).await
}

/// Update the bundled standalone yt-dlp via its built-in self-updater
/// (`yt-dlp -U --update-to nightly`), then report the resulting version. No
/// Python required. YouTube breaks yt-dlp often, so a one-click refresh keeps
/// downloads working without a full app rebuild; nightly carries the fixes first.
#[tauri::command]
async fn update_ytdlp(bins: tauri::State<'_, BinariesDir>) -> Cmd<String> {
    let ytdlp = bins.0.join("yt-dlp-x86_64-pc-windows-msvc.exe");
    tauri::async_runtime::spawn_blocking(move || update_ytdlp_blocking(&ytdlp))
        .await
        .map_err(|e| IpcError::new(IpcErrorKind::Engine, format!("tâche interrompue: {e}")))?
}

fn update_ytdlp_blocking(ytdlp: &std::path::Path) -> Result<String, IpcError> {
    use std::os::windows::process::CommandExt as _;
    const NO_WINDOW: u32 = 0x0800_0000;

    let out = std::process::Command::new(ytdlp)
        .args(["-U", "--update-to", "nightly"])
        .creation_flags(NO_WINDOW)
        .output()
        .map_err(|e| {
            IpcError::new(
                IpcErrorKind::Engine,
                format!("yt-dlp introuvable ({e})."),
            )
        })?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        let reason = stderr.lines().last().unwrap_or("échec de la mise à jour");
        return Err(IpcError::new(
            IpcErrorKind::Engine,
            format!("La mise à jour a échoué : {reason}"),
        ));
    }

    let version = std::process::Command::new(ytdlp)
        .arg("--version")
        .creation_flags(NO_WINDOW)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|| "inconnue".to_string());
    Ok(version)
}

/// Tauri command the injected script calls (`invoke('report_detected_url', ...)`)
/// for every candidate URL it observes. Routed to the right extraction session
/// via the calling window's label.
#[tauri::command]
fn report_detected_url(
    window: tauri::WebviewWindow,
    registry: tauri::State<'_, Arc<ExtractorRegistry>>,
    url: String,
    title: Option<String>,
    thumbnail: Option<String>,
    duration_secs: Option<u32>,
) {
    registry.report(window.label(), url, title, thumbnail, duration_secs);
}

/// Locate a Node.js runtime for yt-dlp's `--js-runtimes`. Returns a spec like
/// `node:C:\Program Files\nodejs\node.exe` (explicit path preferred so it
/// works no matter what PATH the app inherited), or bare `node` if only PATH
/// resolution is possible, or `None` if Node can't be found (YouTube won't
/// work, but every other site still does).
fn resolve_node_runtime() -> Option<String> {
    // Standard Windows install location.
    if let Some(pf) = std::env::var_os("ProgramFiles") {
        let p = std::path::Path::new(&pf).join("nodejs").join("node.exe");
        if p.exists() {
            return Some(format!("node:{}", p.display()));
        }
    }
    // Otherwise scan PATH for node.exe.
    if let Some(paths) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&paths) {
            if dir.join("node.exe").exists() {
                return Some(format!("node:{}", dir.join("node.exe").display()));
            }
        }
    }
    None
}

/// The directory holding the bundled runtime binaries (yt-dlp, ffmpeg, deno).
/// In an installed build they ship as Tauri resources under the app's resource
/// dir; in `cargo tauri dev` there's no resource dir, so fall back to the source
/// tree. Resolving this correctly is what makes the app work on a *fresh* PC:
/// `CARGO_MANIFEST_DIR` only exists on the machine that compiled it.
fn resolve_binaries_dir(app: &tauri::App) -> std::path::PathBuf {
    if let Ok(res) = app.path().resource_dir() {
        let d = res.join("binaries");
        if d.join("yt-dlp-x86_64-pc-windows-msvc.exe").exists() {
            return d;
        }
    }
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("binaries")
}

/// The bundled browser-extension folder (Tauri resource in release, source tree
/// in dev).
fn resolve_extension_dir(app: &tauri::App) -> std::path::PathBuf {
    if let Ok(res) = app.path().resource_dir() {
        let d = res.join("extension");
        if d.join("manifest.json").exists() {
            return d;
        }
    }
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("extension")
}

/// JS runtime spec for yt-dlp's `--js-runtimes` (YouTube signature solving).
/// Prefers the bundled Deno (self-contained — no system Node required), then a
/// Node already on the machine, else `None` (YouTube won't work, other sites do).
fn resolve_js_runtime(binaries_dir: &std::path::Path) -> Option<String> {
    let deno = binaries_dir.join("deno.exe");
    if deno.exists() {
        return Some(format!("deno:{}", deno.display()));
    }
    resolve_node_runtime()
}

/// Directory holding the FFmpeg the engine should use (`--ffmpeg-location`).
/// Prefers a *full* FFmpeg found on the machine (one whose `ffmpeg.exe` reports
/// an AAC decoder — required to remux HLS `.ts` audio into mp4), falling back to
/// the bundled minimal build if no full one is found. `None` only if there's no
/// FFmpeg anywhere (unusual — the bundled one normally exists).
fn resolve_ffmpeg_dir(bundled_dir: &std::path::Path) -> Option<std::path::PathBuf> {
    // Candidate directories, in priority order: known full installs, then any
    // ffmpeg on PATH. Each is used only if it actually has AAC-decode support.
    let mut candidates: Vec<std::path::PathBuf> = Vec::new();

    if let Some(pf) = std::env::var_os("ProgramFiles") {
        candidates.push(std::path::Path::new(&pf).join("Shutter Encoder").join("Library"));
    }
    // CapCut ships a full FFmpeg under a versioned Apps/<version> dir — take the
    // newest present.
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        let apps = std::path::Path::new(&local).join("CapCut").join("Apps");
        if let Ok(entries) = std::fs::read_dir(&apps) {
            let mut dirs: Vec<std::path::PathBuf> =
                entries.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
            dirs.sort(); // version strings sort roughly chronologically
            candidates.extend(dirs.into_iter().rev());
        }
    }
    if let Some(paths) = std::env::var_os("PATH") {
        candidates.extend(std::env::split_paths(&paths));
    }

    for dir in &candidates {
        if dir.join("ffmpeg.exe").exists() && ffmpeg_can_decode_aac(&dir.join("ffmpeg.exe")) {
            tracing::info!(ffmpeg = %dir.display(), "using full FFmpeg");
            return Some(dir.clone());
        }
    }

    // No full FFmpeg found — fall back to the bundled (minimal) one if present.
    if bundled_dir.join("ffmpeg.exe").exists() {
        tracing::warn!(
            "no full FFmpeg found; using bundled minimal build (HLS audio remux may fail)"
        );
        Some(bundled_dir.to_path_buf())
    } else {
        None
    }
}

/// Whether an `ffmpeg.exe` has an AAC *decoder* — the minimal bundled build is
/// encode-only, so its `-codecs` output lists no `decoders: aac`, while a full
/// build does. Distinguishes "can remux HLS audio" from "cannot".
fn ffmpeg_can_decode_aac(ffmpeg: &std::path::Path) -> bool {
    use std::os::windows::process::CommandExt as _;
    const NO_WINDOW: u32 = 0x0800_0000;
    std::process::Command::new(ffmpeg)
        .args(["-hide_banner", "-codecs"])
        .creation_flags(NO_WINDOW)
        .output()
        .map(|o| {
            String::from_utf8_lossy(&o.stdout).contains("decoders: aac")
                || String::from_utf8_lossy(&o.stderr).contains("decoders: aac")
        })
        .unwrap_or(false)
}

/// Fire off a dispatch pass in the background; callers don't wait on it.
fn dispatch(scheduler: &TaskScheduler) {
    let scheduler = scheduler.clone();
    tauri::async_runtime::spawn(async move {
        let _ = scheduler.run_pending().await;
    });
}

/// Initialise `tracing`, writing to a log file next to the SQLite DB
/// (`%APPDATA%\com.local.videodl\video-dl.log`) so there's a durable record to
/// diagnose issues in a GUI build with no console. Level is `info` by default,
/// overridable via the `RUST_LOG` env var. Best-effort: falls back to stderr.
fn init_tracing() {
    use tracing_subscriber::{fmt, EnvFilter};
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("video_dl=info,warn"));

    if let Some(appdata) = std::env::var_os("APPDATA") {
        let dir = std::path::Path::new(&appdata).join("com.local.videodl");
        let _ = std::fs::create_dir_all(&dir);
        if let Ok(file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(dir.join("video-dl.log"))
        {
            let _ = fmt()
                .with_env_filter(filter)
                .with_ansi(false)
                .with_writer(std::sync::Mutex::new(file))
                .try_init();
            return;
        }
    }
    let _ = fmt().with_env_filter(filter).try_init();
}

/// Bootstrap the SQLite-backed state, self-healing a corrupt database file. A
/// hard crash or force-kill mid-write can leave the DB "malformed"; rather than
/// panicking on every launch forever (a boot loop), back the bad file up and
/// recreate a fresh one. History/settings are lost, but the app starts — far
/// better than being permanently wedged. Non-corruption errors still panic
/// (they indicate a real bug, not a recoverable state).
fn bootstrap_with_recovery(db_path: &std::path::Path, probe: Arc<HttpMediaProbe>) -> AppState {
    let path_str = db_path.to_string_lossy().to_string();
    match tauri::async_runtime::block_on(AppState::bootstrap_sqlite(&path_str, probe.clone())) {
        Ok(state) => state,
        Err(e) if is_db_corruption(&e.to_string()) => {
            let stamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let backup = db_path.with_extension(format!("sqlite.corrupt-{stamp}"));
            tracing::error!(
                error = %e,
                backup = %backup.display(),
                "SQLite DB corrupt — backing up and recreating a fresh one",
            );
            let _ = std::fs::rename(db_path, &backup);
            // A stale WAL/SHM against a freshly created DB is itself corruption.
            let _ = std::fs::remove_file(format!("{path_str}-wal"));
            let _ = std::fs::remove_file(format!("{path_str}-shm"));
            tauri::async_runtime::block_on(AppState::bootstrap_sqlite(&path_str, probe))
                .expect("bootstrap failed even after recreating the database")
        }
        Err(e) => panic!("bootstrap failed: {e}"),
    }
}

/// Whether an error message indicates an unreadable/corrupt SQLite file (vs a
/// transient or logic error that recreating the DB wouldn't fix).
fn is_db_corruption(msg: &str) -> bool {
    let m = msg.to_lowercase();
    m.contains("malformed")
        || m.contains("disk image")
        || m.contains("not a database")
        || m.contains("database corrupt")
        || m.contains("file is encrypted")
}

pub fn run() {
    init_tracing();
    tracing::info!("Video DL starting");
    tauri::Builder::default()
        // Must be the first plugin registered. A relaunch (double-click the
        // shortcut while the app is already running) hits this callback
        // instead of starting a second process — focus/restore the existing
        // window rather than piling up duplicates the user can't find.
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            show_main_window(app);
        }))
        // Closing the window minimises to the system tray instead of quitting;
        // the tray menu's "Quitter" is the real exit. Lets downloads keep
        // running in the background without a taskbar window.
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            let db_path = app
                .path()
                .app_data_dir()
                .expect("app data dir")
                .join("video-dl.sqlite");
            std::fs::create_dir_all(db_path.parent().unwrap()).ok();

            let probe = Arc::new(HttpMediaProbe::new());
            let state = bootstrap_with_recovery(&db_path, probe.clone());

            // Bridge the internal event bus -> Tauri events (backend -> frontend).
            let mut rx = state.event_bus.subscribe();
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                while let Ok(event) = rx.recv().await {
                    let ipc = IpcEvent::from(event);
                    let channel = ipc.channel();
                    let payload = match &ipc {
                        IpcEvent::MediaDetected(p) => serde_json::to_value(p),
                        IpcEvent::DownloadCreated(p) => serde_json::to_value(p),
                        IpcEvent::TaskStateChanged(p) => serde_json::to_value(p),
                        IpcEvent::Progress(p) => serde_json::to_value(p),
                        IpcEvent::BrowsingContextChanged(p) => serde_json::to_value(p),
                    };
                    if let Ok(payload) = payload {
                        let _ = handle.emit(channel, payload);
                    }
                }
            });

            // Bundled runtime binaries (yt-dlp, ffmpeg, deno). In an installed
            // build they're shipped as Tauri resources next to the exe; in
            // `cargo tauri dev` they're read from the source tree.
            let binaries_dir = resolve_binaries_dir(app);
            let ytdlp_path = binaries_dir.join("yt-dlp-x86_64-pc-windows-msvc.exe");
            app.manage(BinariesDir(binaries_dir.clone()));
            app.manage(ExtensionDir(resolve_extension_dir(app)));

            // FFmpeg lets yt-dlp merge YouTube's separate audio/video streams
            // and remux HLS (anime-sama, ...) into mp4. The bundled build is now
            // a *full* static ffmpeg (AAC decoder + https), so HLS remux works
            // out of the box on any PC. We still let a full FFmpeg already on the
            // machine (Shutter Encoder, CapCut, PATH) win if present, and fall
            // back to the bundled one — which is itself full.
            let ffmpeg_dir = resolve_ffmpeg_dir(&binaries_dir);
            // Whether the resolved FFmpeg can remux HLS audio — surfaced to the
            // Settings banner (should always be true now that the bundle is full).
            let ffmpeg_full = ffmpeg_dir
                .as_ref()
                .map(|d| ffmpeg_can_decode_aac(&d.join("ffmpeg.exe")))
                .unwrap_or(false);
            app.manage(FfmpegFull(ffmpeg_full));

            // JS runtime for yt-dlp's YouTube signature solving (mandatory for
            // YouTube since 2025). Prefer the bundled Deno (self-contained — no
            // system Node needed); fall back to a Node on the machine.
            let js_runtime = resolve_js_runtime(&binaries_dir);

            let download_dir = app
                .path()
                .download_dir()
                .unwrap_or_else(|_| std::env::temp_dir());

            // CompositeEngine routes each download between yt-dlp (site
            // extraction) and a direct HTTP stream (plain files yt-dlp's
            // generic extractor would refuse) — see RegistryPlanner.
            let resolve_playlist = ResolvePlaylist::new(Arc::new(YtDlpPlaylistResolver::new(
                ytdlp_path.clone(),
                js_runtime.clone(),
            )));
            let engine = Arc::new(CompositeEngine::new(
                YtDlpEngine::new(ytdlp_path, ffmpeg_dir).with_js_runtime(js_runtime),
                HttpDownloadEngine::new(),
            ));
            let planner = Arc::new(RegistryPlanner::new(
                ProviderRegistry::with_defaults(),
                download_dir.to_string_lossy().to_string(),
                DownloadOptions::default(),
            ));
            let scheduler = TaskScheduler::new(
                state.download_repo.clone(),
                engine,
                planner,
                state.event_publisher.clone(),
                2,
                state.running.clone(),
            );

            // Resume any tasks that crash-recovery re-queued.
            dispatch(&scheduler);

            // Page extraction: hidden/visible window + injected fetch/XHR hooks,
            // feeding the same probe + event pipeline as direct-link detection.
            let extractor_registry = ExtractorRegistry::new();
            let page_extractor = Arc::new(TauriPageExtractor::new(
                app.handle().clone(),
                extractor_registry.clone(),
            ));
            let extract_from_page = ExtractFromPage::new(
                page_extractor,
                probe.clone(),
                state.event_publisher.clone(),
            );

            // Live detection (ADR 4.6): local WebSocket bridge for the Opera GX
            // extension. Generates a token on first run, then serves in the
            // background. A bind failure is logged, not fatal — the rest of the
            // app works without it.
            if let Ok(mut settings) = tauri::async_runtime::block_on(state.get_settings.execute()) {
                if settings.extension_token.is_empty() {
                    settings.extension_token = uuid::Uuid::new_v4().to_string();
                    let _ = tauri::async_runtime::block_on(
                        state.update_settings.execute(settings.clone()),
                    );
                }
                if settings.extension_enabled {
                    let ingest = Arc::new(IngestDetection::new(
                        probe.clone(),
                        state.event_publisher.clone(),
                    ));
                    let bridge = ExtensionWsBridge::new(
                        ingest,
                        settings.extension_port,
                        settings.extension_token.clone(),
                    );
                    // Keep the outbound handle so the widget's refresh button
                    // can push a rescan back to the extension.
                    app.manage(bridge.handle());
                    tauri::async_runtime::spawn(async move {
                        if let Err(e) = bridge.serve().await {
                            tracing::error!("extension WS bridge stopped: {e}");
                        }
                    });
                } else {
                    app.manage(ExtensionWsHandle::default());
                }
            } else {
                app.manage(ExtensionWsHandle::default());
            }

            app.manage(extractor_registry);
            app.manage(extract_from_page);
            app.manage(resolve_playlist);
            app.manage(state);
            app.manage(SchedulerHandle(scheduler));

            // System-tray icon + menu (paired with the close-to-hide handler
            // above). Left-click or "Ouvrir" restores the window; "Quitter" is
            // the only real exit.
            let show_item = MenuItem::with_id(app, "show", "Ouvrir Video DL", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "Quitter", true, None::<&str>)?;
            let tray_menu = Menu::with_items(app, &[&show_item, &quit_item])?;
            let mut tray = TrayIconBuilder::new()
                .tooltip("Video DL")
                .menu(&tray_menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => show_main_window(app),
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        show_main_window(tray.app_handle());
                    }
                });
            if let Some(icon) = app.default_window_icon().cloned() {
                tray = tray.icon(icon);
            }
            tray.build(app)?;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            probe_url,
            extract_from_page,
            open_interactive_extraction,
            start_download,
            resolve_playlist,
            start_playlist_download,
            cancel_download,
            pause_download,
            resume_download,
            list_active,
            list_history,
            remove_download,
            retry_download,
            delete_history_entry,
            open_containing_folder,
            open_download_folder,
            reveal_extension_folder,
            open_external,
            rescan_extension,
            extension_connected,
            ffmpeg_is_full,
            get_settings,
            update_settings,
            update_ytdlp,
            report_detected_url
        ])
        .run(tauri::generate_context!())
        .expect("error while running Video DL");
}
