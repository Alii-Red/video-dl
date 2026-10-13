# Video DL

Application desktop **personnelle** de détection et de téléchargement de médias
(inspirée de Video DownloadHelper). Locale, privée, non distribuée.

Elle repère les vidéos chargées dans le navigateur (via une extension Opera GX,
l'extraction de page, ou un import manuel d'URL) et les télécharge avec
yt-dlp / FFmpeg. Un widget compact *always-on-top* permet de lancer un
téléchargement en un clic pendant la navigation.

## Fonctionnalités

- **Détection live** via une extension Opera GX (canal WebSocket local) :
  flux HLS/DASH sniffés, pages yt-dlp-natives (YouTube…) offertes directement,
  liens directs KVS/tube extraits du HTML.
- **Extraction de page** (hook fetch/XHR injecté) et **import manuel** d'URL.
- Téléchargement via un **moteur composite** : yt-dlp (extraction de site) ou
  HTTP direct (fichiers que l'extracteur générique de yt-dlp refuse).
- Providers : `generic`, `youtube`, `reddit`, `vimeo`.
- **Widget volatile** always-on-top, **réduction au tray**, **notifications**
  de fin de téléchargement.
- Qualité par téléchargement, titre de fichier éditable, historique,
  « ouvrir le dossier », réglages persistants.
- Téléchargements HLS parallélisés (64 fragments par défaut) et
  **auto-détection** d'un FFmpeg complet sur la machine.

## Stack

- **Backend** : Rust, Tokio, Tauri v2, SQLx + SQLite, reqwest, tracing.
  Architecture *Clean* en 3 couches (domain pur ← application : ports/use cases
  ← infrastructure), injection de dépendances manuelle via `AppState` +
  `Arc<dyn Trait>`.
- **Frontend** : React 19, TypeScript, Vite, Tailwind v4, TanStack Query,
  Zustand. Organisation *feature-first*, client IPC typé (bindings générés par
  [ts-rs]).
- **Outils externes** : yt-dlp (+ un runtime JS Node pour la signature YouTube)
  et FFmpeg.

## Structure

```
apps/
  src-tauri/     backend Rust + shell Tauri (domain / application / infrastructure / ipc / desktop)
  frontend/      UI React (features/, stores/, lib/, components/)
  extension/     extension Opera GX MV3 (background.js, content-script.js, options)
docs/            ADR d'architecture, BUILD.md, STATUS.md, INTEGRATION-TAURI.md
scripts/         dev.ps1 (toolchain GNU) et dev-msvc.ps1 (toolchain MSVC)
```

## Prérequis

- **Rust** (toolchain MSVC recommandé ; GNU possible en repli).
- **Visual Studio Build Tools** (MSVC) + WebView2 pour le build Tauri.
- **Node.js** (sert aussi de runtime JS à yt-dlp pour YouTube).
- **yt-dlp** et un **FFmpeg complet** accessibles (voir ci-dessous).

> Détails et contournements spécifiques (chemin utilisateur accentué, linker…)
> dans [`docs/BUILD.md`](docs/BUILD.md).

## Build & lancement

```powershell
# 1. Dépendances frontend
cd apps/frontend; npm install; cd ../..

# 2. Build de production (frontend embarqué) + installeur NSIS
.\scripts\dev-msvc.ps1 tauri build
# puis installer : target-msvc\release\bundle\nsis\Video DL_..._x64-setup.exe /S

# Développement (serveur Vite + fenêtre Tauri)
.\scripts\dev-msvc.ps1 tauri dev

# Tests (régénère aussi les bindings ts-rs)
.\scripts\dev-msvc.ps1 test
```

> ⚠️ Toujours lancer l'app **installée** (menu Démarrer / `%LOCALAPPDATA%\Video DL`),
> pas l'exe brut de `target-msvc\debug` (il chargerait son UI depuis le serveur
> de dev → fenêtre noire).

### FFmpeg

Le FFmpeg embarqué est minimal (merge DASH YouTube uniquement). Pour remuxer
l'audio des flux HLS (anime-sama…), l'app **auto-détecte** un FFmpeg complet
déjà installé (Shutter Encoder, CapCut, ou `ffmpeg` dans le PATH). Une bannière
dans les Réglages prévient si aucun FFmpeg complet n'est trouvé.

## Extension Opera GX

1. `opera://extensions` → activer le mode développeur → *Charger une extension
   non empaquetée* → dossier `apps/extension/`.
2. Ouvrir les options de l'extension, y coller le **port** et le **token**
   affichés dans **Réglages** de l'app (bouton copier).
3. Après toute modif de `background.js` / `content-script.js` / `manifest.json`,
   **recharger l'extension** (bouton ↻).

## Travailler depuis plusieurs PC

Les bindings ts-rs (`apps/frontend/src/types/*.ts`) sont versionnés pour qu'un
clone frais builde directement. S'ils changent, `cargo test` les régénère — à
recommiter.

```bash
git clone https://github.com/Alii-Red/video-dl.git
# sur chaque PC :
git pull                                   # récupérer le travail de l'autre PC
git add -A && git commit -m "..." && git push
```

## Licence

Projet personnel, non distribué. yt-dlp et FFmpeg sont redistribués selon leurs
licences respectives.

[ts-rs]: https://github.com/Aleph-Alpha/ts-rs
