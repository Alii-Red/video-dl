# Video DL

> ## ⬇️ [Télécharger Video DL (Windows)](https://github.com/Alii-Red/video-dl/releases/latest)
>
> Cliquez le lien ci-dessus, puis téléchargez **`Video DL_..._x64-setup.exe`**
> dans la section **Assets**. L'installeur contient tout (yt-dlp, FFmpeg, Deno) —
> rien d'autre à installer.
>
> 📖 Besoin d'aide ? → **[Guide d'installation pas à pas](INSTALL.md)**

---

Application desktop **personnelle** de détection et de téléchargement de médias
(inspirée de Video DownloadHelper). Locale, privée, non distribuée.

Pensée avant tout pour **[anime-sama](https://anime-sama.to)** : lance un
épisode, il apparaît dans l'app, un clic pour le télécharger. Elle gère aussi
YouTube et, plus généralement, les vidéos chargées dans le navigateur (via une
extension, l'extraction de page, ou un import manuel d'URL), le tout avec
yt-dlp / FFmpeg. Un widget compact *always-on-top* permet de télécharger en un
clic pendant la navigation.

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

## Installeur « tout compris »

L'installeur de production **embarque toutes les ressources runtime** —
yt-dlp (exe autonome), un **FFmpeg complet**, et **Deno** (moteur JS pour
YouTube). Sur le PC cible : **ni Python, ni Node, ni FFmpeg à installer**, et
WebView2 s'installe tout seul si besoin. Voir
[`INSTALL.md`](INSTALL.md) → *Option A*.

## Prérequis (pour compiler soi-même)

- **Rust** (toolchain MSVC recommandé ; GNU possible en repli).
- **Visual Studio Build Tools** (MSVC) pour le build Tauri.
- **Node.js** (build du frontend).
- `cargo install tauri-cli --version "^2"`.

> Plus besoin de Python ni de FFmpeg côté dev : les binaires runtime sont
> récupérés par `scripts/fetch-runtime.ps1`. Détails et contournements
> (chemin utilisateur accentué, linker…) dans [`docs/BUILD.md`](docs/BUILD.md).

## Build & lancement

```powershell
# 1. Binaires runtime (yt-dlp, ffmpeg, deno) — non versionnés, ~250 Mo
.\scripts\fetch-runtime.ps1

# 2. Dépendances frontend
cd apps/frontend; npm install; cd ../..

# 3. Build de production (frontend + binaires embarqués) + installeur NSIS
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

Un **FFmpeg complet est embarqué** dans l'installeur (remux HLS + merge DASH).
L'app privilégie un FFmpeg complet déjà présent sur la machine (Shutter Encoder,
CapCut, PATH) s'il y en a un, et retombe sinon sur celui embarqué. Une bannière
dans les Réglages prévient dans le cas improbable où aucun FFmpeg complet n'est
disponible.

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
# première fois sur un PC : récupérer les binaires runtime
# .\scripts\fetch-runtime.ps1
# puis, à chaque fois :
git pull                                   # récupérer le travail de l'autre PC
git add -A && git commit -m "..." && git push
```

## Licence

Projet personnel, non distribué. yt-dlp et FFmpeg sont redistribués selon leurs
licences respectives.

[ts-rs]: https://github.com/Aleph-Alpha/ts-rs
