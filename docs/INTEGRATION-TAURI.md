# Assemblage Tauri (dernière étape) — Video DL

> **Statut : étape manuelle, requiert MSVC.**
> Le backend (lib `video_dl`) et le frontend sont complets et testés. Cette étape
> les relie dans une app Tauri v2. Elle est documentée plutôt que committée car
> elle ne compile pas dans l'environnement de dev automatique utilisé jusqu'ici :
> Tauri sur Windows exige **MSVC Build Tools** (élévation UAC) + **WebView2**, et
> ne supporte pas le toolchain **GNU** de secours. Une fois MSVC installé, suis ce
> guide — tout le code métier appelé ici existe déjà (`ipc::commands`, `AppState`).

## 0. Pré-requis

```powershell
# Build Tools Visual C++ (linker MSVC) — nécessite l'élévation UAC
winget install Microsoft.VisualStudio.2022.BuildTools -e `
  --override "--quiet --wait --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
# Toolchain MSVC (par défaut sur Windows)
rustup default stable-x86_64-pc-windows-msvc
# CLI Tauri
npm --prefix apps/frontend install
cargo install tauri-cli --version "^2"
```

Avec MSVC, le contournement GNU/`é` de `docs/BUILD.md` devient inutile : `link.exe`
gère les chemins non-ASCII. Tu peux builder avec `cargo`/`tauri` directement.

## 1. Sidecars yt-dlp + FFmpeg

Place les binaires dans `apps/src-tauri/binaries/`, suffixés par la *target triple*
(convention sidecar Tauri) :

```
apps/src-tauri/binaries/yt-dlp-x86_64-pc-windows-msvc.exe
apps/src-tauri/binaries/ffmpeg-x86_64-pc-windows-msvc.exe
```

## 2. `apps/src-tauri/Cargo.toml` — ajouts

```toml
[build-dependencies]
tauri-build = { version = "2", features = [] }

[dependencies]
# ... (dépendances existantes) ...
tauri = { version = "2", features = [] }
tauri-plugin-shell = "2"
serde_json = "1"

[[bin]]
name = "video-dl"
path = "src/main.rs"
```

## 3. `apps/src-tauri/build.rs`

```rust
fn main() {
    tauri_build::build();
}
```

## 4. `apps/src-tauri/tauri.conf.json`

```json
{
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "Video DL",
  "version": "0.1.0",
  "identifier": "com.local.videodl",
  "build": {
    "frontendDist": "../frontend/dist",
    "devUrl": "http://localhost:1420",
    "beforeDevCommand": "npm --prefix ../frontend run dev",
    "beforeBuildCommand": "npm --prefix ../frontend run build"
  },
  "app": {
    "windows": [{ "title": "Video DL", "width": 1100, "height": 720 }],
    "security": { "csp": null }
  },
  "bundle": {
    "active": true,
    "targets": "all",
    "externalBin": ["binaries/yt-dlp", "binaries/ffmpeg"]
  }
}
```

## 5. `apps/src-tauri/src/main.rs`

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    video_dl::desktop::run();
}
```

## 6. `apps/src-tauri/src/desktop.rs` — commandes + bridge d'events

Ce module est le seul qui connaît Tauri. Il enveloppe les handlers réutilisables
déjà présents dans `ipc::commands` (testés, Tauri-free) et branche le bus d'events
interne sur `emit`. Ajoute `pub mod desktop;` dans `lib.rs`.

```rust
use tauri::{Emitter, Manager};

use crate::ipc::commands;
use crate::ipc::dto::{MediaSourceDto, StartDownloadArgs, StartDownloadResponse, TaskDto};
use crate::ipc::error::IpcError;
use crate::ipc::events::IpcEvent;
use crate::state::AppState;

type Cmd<T> = Result<T, IpcError>;

#[tauri::command]
async fn probe_url(state: tauri::State<'_, AppState>, url: String) -> Cmd<MediaSourceDto> {
    commands::probe_url(&state, url).await
}

#[tauri::command]
async fn start_download(
    state: tauri::State<'_, AppState>,
    args: StartDownloadArgs,
) -> Cmd<StartDownloadResponse> {
    commands::start_download(&state, args).await
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
async fn resume_download(state: tauri::State<'_, AppState>, task_id: String) -> Cmd<()> {
    commands::resume_download(&state, task_id).await
}

#[tauri::command]
async fn list_history(state: tauri::State<'_, AppState>) -> Cmd<Vec<TaskDto>> {
    commands::list_history(&state).await
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            // DB path under the app data dir.
            let db_path = app
                .path()
                .app_data_dir()
                .expect("app data dir")
                .join("video-dl.sqlite");
            std::fs::create_dir_all(db_path.parent().unwrap()).ok();

            // Real detection probe (HTTP + manifest parsing).
            let probe = std::sync::Arc::new(crate::infrastructure::network::HttpMediaProbe::new());

            // Bootstrap the app (connect SQLite, run crash recovery).
            let state = tauri::async_runtime::block_on(AppState::bootstrap_sqlite(
                db_path.to_string_lossy().as_ref(),
                probe,
            ))
            .expect("bootstrap failed");

            // Bridge the internal event bus → Tauri events (backend → frontend).
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
                    };
                    if let Ok(payload) = payload {
                        let _ = handle.emit(channel, payload);
                    }
                }
            });

            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            probe_url,
            start_download,
            cancel_download,
            pause_download,
            resume_download,
            list_history
        ])
        .run(tauri::generate_context!())
        .expect("error while running Video DL");
}
```

> Les noms de commandes (`probe_url`, `start_download`, …) et les canaux d'events
> (`download://progress`, `task://state-changed`, …) correspondent déjà à ceux
> attendus par le client frontend dans `apps/frontend/src/lib/ipc.ts`. Tauri v2
> convertit automatiquement les clés JS camelCase (`taskId`) en params Rust
> snake_case (`task_id`).

## 7. Câbler le scheduler (exécution réelle des tâches)

`start_download` crée une tâche `Queued`. Pour l'exécuter, construis un
`TaskScheduler` (voir `infrastructure::scheduler`) avec :

- le même repo SQLite (partage l'état),
- `YtDlpEngine::new(<sidecar yt-dlp>, Some(<dir ffmpeg>))`,
- un `RegistryPlanner::new(ProviderRegistry::with_defaults(), <dossier de téléchargement>, options)`,
- le bus d'events.

Puis appelle `scheduler.run_pending()` (au boot après recovery, et après chaque
`start_download`). Le chemin du sidecar se résout via
`app.path().resolve("binaries/yt-dlp", BaseDirectory::Resource)` ou l'API
`tauri_plugin_shell` sidecar. C'est le seul morceau de logique non encore branché
côté desktop ; tout ce qu'il appelle (`TaskScheduler`, `YtDlpEngine`,
`RegistryPlanner`) est déjà implémenté et testé.

## 8. Lancer

```powershell
cargo tauri dev      # depuis apps/src-tauri, ou `cargo tauri dev` à la racine avec le bon chemin
cargo tauri build    # bundle de production
```
