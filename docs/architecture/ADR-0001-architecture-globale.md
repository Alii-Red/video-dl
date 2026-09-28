# ADR-0001 — Architecture globale (Video DL — usage personnel, local)

> **Statut : VALIDÉ (2026-07-18) — implémentation en cours.**
> **Révision 2026-07-19** : ajout de deux exigences produit (bascule de mode
> fenêtre standard/volatile + détection live via extension navigateur), et
> **abandon du proxy MITM** (Phase 9) remplacé par l'extension. Voir Parties 1.8,
> 4.6, la roadmap révisée et « Décisions validées » en fin de document.
> Voir aussi **[../STATUS.md](../STATUS.md)** pour l'avancement (Phases 1-7 faites
> et testées, app Tauri fonctionnelle, extraction de page validée en réel).
> **Contexte :** application desktop strictement personnelle, locale, non distribuée.
> **Auteur :** Architecte / Tech Lead (session de conception)
> **Date :** 2026-07-17

---

## Note de cadrage (à lire avant tout)

Ce document est une **étude d'architecture**, pas du code métier. Aucune implémentation
n'est produite tant que tu n'as pas validé les décisions ci-dessous.

Principe directeur imposé par ton contexte : **pragmatique et propre, pas enterprise.**
Chaque fois qu'un pattern « lourd » (CQRS, Event Sourcing, DI framework, hexagonal strict…)
est mentionné, je précise s'il apporte une **vraie valeur pour un dev solo en local** ou
si c'est de la sur-ingénierie à éviter. Les sections « sécurité multi-utilisateurs »,
« durcissement anti-attaquant », « télémétrie », « distribution » sont **volontairement
absentes** — elles ne s'appliquent pas.

---

## Partie 1 — Architecture générale

### 1.1 Vue d'ensemble

L'application se décompose en deux processus Tauri classiques :

- **Frontend** (WebView) : React/TS. Pur affichage + orchestration UI. Aucune logique
  métier lourde, aucun accès disque/réseau direct.
- **Backend Rust** (processus natif Tauri) : tout le métier, l'accès réseau, la base,
  le pilotage de yt-dlp/FFmpeg, et le **serveur WebSocket local** qui reçoit les
  détections de l'extension navigateur (voir Partie 4.6).

Communication exclusivement via **IPC Tauri** (commands + events). Le frontend ne parle
jamais à SQLite ni au réseau directement.

### 1.2 Couches (backend Rust)

On adopte une **Clean Architecture allégée à 3 couches** (pas 4+). Pour un projet solo,
3 couches suffisent et évitent la prolifération de traits/adapters inutiles.

```
┌─────────────────────────────────────────────────────────┐
│  PRESENTATION / IPC  (commands Tauri, events, DTO ↔ TS)   │  ← dépend de application
├─────────────────────────────────────────────────────────┤
│  APPLICATION  (use cases, orchestration, ports/traits)    │  ← dépend de domain
├─────────────────────────────────────────────────────────┤
│  DOMAIN  (entities, value objects, règles pures)          │  ← ne dépend de RIEN
└─────────────────────────────────────────────────────────┘
              ▲
              │ implémente les ports (traits) définis par application
┌─────────────────────────────────────────────────────────┐
│  INFRASTRUCTURE  (SQLx, reqwest, WS bridge, yt-dlp, FFmpeg,│
│                   providers, event bus, scheduler)         │
└─────────────────────────────────────────────────────────┘
```

**Règle de dépendance (le cœur de Clean Archi) :** les flèches pointent vers l'intérieur.
`domain` ne connaît personne. `application` définit des **ports** (traits Rust :
`DownloadRepository`, `MediaProbe`, `DownloadEngine`…). `infrastructure` **implémente**
ces ports. `ipc` appelle les use cases de `application`.

Concrètement, l'inversion de dépendance se fait avec des **traits Rust + `Arc<dyn Trait>`**.
Pas besoin d'un framework de DI : on câble tout à la main dans un `AppState` au démarrage
(composition root). C'est le pattern le plus lisible en Rust solo.

### 1.3 Responsabilités par couche

| Couche | Responsabilité | Exemples |
|---|---|---|
| **Domain** | Entités + règles pures, sans I/O | `Download`, `MediaSource`, `Quality`, `TaskState`, transitions d'état valides |
| **Application** | Use cases orchestrant le domaine, définit les ports | `StartDownload`, `ProbeUrl`, `EnqueueTask`, `CancelDownload` |
| **Infrastructure** | Implémentations concrètes des ports | `SqlxDownloadRepository`, `YtDlpEngine`, `HttpMediaProbe`, `ExtensionWsBridge` |
| **IPC/Presentation** | Frontière Tauri, sérialisation, streaming d'events | `#[tauri::command]`, `emit`, mapping erreurs → DTO |

### 1.4 Flux de données (nominal)

```
Sources de détection (toutes convergent vers le même pipeline)
      │
      ├─(a) Extension Opera GX (webRequest) → WS local ──┐   ← principal (Partie 4.6)
      ├─(b) Fenêtre d'extraction interne + hook JS ──────┤→ MediaDetected(event interne)
      ├─(c) Import manuel d'URL ─────────────────────────┤
      └─(d) Probe direct d'un lien média ────────────────┘
                                                   │
                                                   ▼
                            Application::ProbeUrl (use case)
                                                   │
                     Infrastructure: probe HTTP + parse M3U8/MPD
                                                   │
                                                   ▼
                            Domain: MediaSource { qualités, formats }
                                                   │
                              IPC event → Frontend affiche « médias détectés »
                                                   │
                    Utilisateur clique « Télécharger »  → IPC command
                                                   ▼
                    Application::StartDownload → crée Task en SQLite (état Queued)
                                                   │
                              TaskScheduler prend la tâche → YtDlpEngine
                                                   │
                    lecture stdout/stderr → progression → IPC events (streaming)
                                                   ▼
                    Fin → FFmpeg merge si besoin → état Completed → History
```

### 1.5 Flux d'événements

Deux bus distincts, à ne pas confondre :

1. **Event bus interne Rust** (`tokio::sync::broadcast` ou `mpsc`) : découple les
   producteurs (proxy, engine, scheduler) des consommateurs (use cases). C'est là que
   « une valeur réelle » existe pour un projet solo : ça évite le couplage direct
   proxy → downloader.
2. **Events IPC Tauri** (`app.emit`) : backend → frontend uniquement (progression,
   notifications). Le frontend s'abonne via `listen`.

### 1.6 Répartition frontend / backend

- **Backend** : détection, analyse, décision de qualité, file d'attente, exécution,
  persistance, reprise. **Source de vérité unique.**
- **Frontend** : rien de critique. Il affiche l'état poussé par le backend et envoie des
  intentions (commands). En cas de crash du frontend, aucune donnée n'est perdue :
  tout est en SQLite côté Rust.

### 1.7 Décisions Partie 1 & justifications

| Décision | Pourquoi | Alternative écartée |
|---|---|---|
| Clean Archi **3 couches** (pas 4) | Suffisant en solo, moins de boilerplate | Hexagonal strict → trop de traits/adapters pour rien |
| Ports = **traits Rust + Arc<dyn>** | Idiomatique, testable (mock via trait) | Framework DI (shaku…) → complexité inutile |
| **Repository Pattern** : oui | Isole SQLx, permet tests domaine sans DB | Accès SQLx direct dans use cases → non testable, couplé |
| **DDD léger** : entities + value objects, **pas** d'agrégats formels ni de bounded contexts multiples | Le domaine est simple (1 contexte) | DDD complet → sur-ingénierie |
| **CQRS : NON** (sauf séparation naturelle command/query dans les repos) | 1 seul utilisateur, pas de charge | CQRS + read models → aucun gain ici |
| **Event Sourcing : NON** | Historique simple via table `History` | ES → complexité massive injustifiée |
| Backend = **source de vérité** | Robustesse, reprise après crash | État côté front → perte de données |

### 1.8 Modes de fenêtre (standard / volatile) — révision 2026-07-19

L'app expose **deux modes de présentation d'une seule et même fenêtre** (jamais deux
fenêtres/apps séparées) :

- **Standard** : fenêtre normale, sidebar, navigation Détection/Téléchargements/
  Historique/Réglages (l'existant).
- **Volatile** : petite fenêtre compacte, `alwaysOnTop`, décorations réduites, pensée
  pour rester par-dessus Opera GX pendant la navigation. Affiche uniquement l'essentiel
  (détections live + téléchargements actifs).

**Principe architectural clé** : le mode est **purement une préoccupation de la couche
présentation**. Il ne descend jamais dans domaine / application / IPC. Le state Zustand,
TanStack Query, le contrat IPC, le pipeline de détection sont **identiques** dans les deux
modes — seuls changent (a) les propriétés de la fenêtre Tauri (taille, alwaysOnTop,
décorations, position) mutées **en live** via l'API `window`, et (b) le layout React
rendu. Une détection arrivée en mode volatile est donc immédiatement visible si on bascule
en standard, et inversement (même source de vérité).

Le mode courant + la géométrie mémorisée du mode volatile sont persistés dans les
`Settings` existants (pas de plugin window-state) et relus au boot.

**Réserve technique** : le toggle des `decorations` à chaud sous Windows/WebView2 peut
avoir des hoquets (flash, hide/show). Repli propre : décorations off en volatile + barre
custom (`data-tauri-drag-region`) + fenêtre non-redimensionnable à taille fixe. À valider
à l'implémentation, pas garanti fluide du premier coup.

---

## Partie 2 — Arborescence du projet

```
video-dl/
├── docs/
│   └── architecture/
│       └── ADR-0001-architecture-globale.md   # ce document
├── package.json                                # workspace racine (scripts tauri)
├── apps/
│   ├── frontend/                               # React/TS/Vite
│   │   ├── index.html
│   │   ├── vite.config.ts
│   │   ├── tailwind.config.ts
│   │   ├── tsconfig.json
│   │   └── src/
│   │       ├── main.tsx
│   │       ├── App.tsx
│   │       ├── app/                            # setup: router, providers, query client
│   │       ├── features/                       # feature-first (voir Partie 8)
│   │       │   ├── detection/
│   │       │   ├── downloads/
│   │       │   ├── history/
│   │       │   ├── queue/
│   │       │   └── settings/
│   │       ├── components/ui/                  # shadcn/ui (primitives)
│   │       ├── lib/                            # helpers, ipc client, query keys
│   │       ├── stores/                         # Zustand (état UI transverse)
│   │       └── types/
│   │           └── ipc.gen.ts                  # types générés depuis Rust (ts-rs)
│   │
│   └── src-tauri/                              # Backend Rust
│       ├── Cargo.toml
│       ├── tauri.conf.json
│       ├── build.rs
│       └── src/
│           ├── main.rs                         # entrée, composition root, AppState
│           ├── lib.rs
│           │
│           ├── domain/                         # COUCHE DOMAIN (zéro dépendance I/O)
│           │   ├── mod.rs
│           │   ├── entities/                   # Download, Task, MediaSource, Playlist
│           │   ├── value_objects/              # Quality, MediaFormat, Progress, Url
│           │   ├── errors.rs                   # erreurs métier (DomainError)
│           │   └── services/                   # règles pures multi-entités
│           │
│           ├── application/                    # COUCHE APPLICATION (use cases + ports)
│           │   ├── mod.rs
│           │   ├── ports/                      # TRAITS = interfaces (repos, engine, probe)
│           │   │   ├── repositories.rs
│           │   │   ├── download_engine.rs
│           │   │   ├── media_probe.rs
│           │   │   └── event_publisher.rs
│           │   ├── use_cases/                  # 1 fichier = 1 use case
│           │   │   ├── probe_url.rs
│           │   │   ├── start_download.rs
│           │   │   ├── cancel_download.rs
│           │   │   ├── pause_resume.rs
│           │   │   └── list_history.rs
│           │   └── dto.rs                      # DTO internes application
│           │
│           ├── infrastructure/                 # COUCHE INFRA (implémente les ports)
│           │   ├── mod.rs
│           │   ├── database/                   # SQLx + migrations + repos concrets
│           │   │   ├── mod.rs
│           │   │   ├── pool.rs
│           │   │   ├── migrations/             # .sql versionnées
│           │   │   └── repositories/           # SqlxDownloadRepository, etc.
│           │   ├── download/                   # moteur yt-dlp + FFmpeg
│           │   │   ├── ytdlp_engine.rs
│           │   │   ├── ytdlp_args.rs
│           │   │   ├── progress_parser.rs      # parse stdout yt-dlp
│           │   │   └── ffmpeg.rs
│           │   ├── network/                    # PARTIE 4 (détection)
│           │   │   ├── mod.rs
│           │   │   ├── proxy/                  # MITM local
│           │   │   │   ├── mod.rs
│           │   │   │   ├── ca.rs               # génération/gestion CA local
│           │   │   │   └── interceptor.rs
│           │   │   ├── inject/                 # script JS d'interception
│           │   │   │   └── hook.js
│           │   │   ├── probe.rs                # HEAD/GET, content-type, ranges
│           │   │   ├── hls.rs                  # parsing M3U8
│           │   │   └── dash.rs                 # parsing MPD non chiffré
│           │   ├── providers/                  # PARTIE 6
│           │   │   ├── mod.rs                  # registry + trait Provider
│           │   │   ├── generic.rs
│           │   │   ├── youtube.rs
│           │   │   ├── reddit.rs
│           │   │   ├── vimeo.rs
│           │   │   ├── twitter.rs
│           │   │   ├── instagram.rs
│           │   │   ├── facebook.rs
│           │   │   └── tiktok.rs
│           │   ├── events/                     # event bus interne (tokio broadcast)
│           │   │   └── bus.rs
│           │   ├── scheduler/                  # file d'attente + concurrence
│           │   │   └── task_scheduler.rs
│           │   └── config/                     # settings, logging (tracing)
│           │       ├── settings.rs
│           │       └── logging.rs
│           │
│           ├── ipc/                            # COUCHE PRESENTATION (Tauri)
│           │   ├── mod.rs
│           │   ├── commands/                   # #[tauri::command] par domaine
│           │   │   ├── downloads.rs
│           │   │   ├── detection.rs
│           │   │   ├── settings.rs
│           │   │   └── history.rs
│           │   ├── events.rs                   # définitions d'events IPC + payloads
│           │   └── error.rs                    # AppError → sérialisable pour le front
│           │
│           ├── plugins/                        # plugins Tauri custom éventuels
│           │
│           └── tests/                          # tests d'intégration (Partie 10)
│               ├── pipeline_download.rs
│               ├── manifest_parsing.rs
│               └── ipc_smoke.rs
```

### 2.1 Rôle de chaque dossier clé

- **`domain/`** : le noyau pur. Si tu supprimes Tauri, SQLite et le réseau, ce dossier
  compile encore. Contient les règles (ex. : transitions d'état d'une tâche).
- **`application/ports/`** : les **contrats** (traits). C'est là que l'inversion de
  dépendance vit. Le domaine ne les connaît pas ; l'infra les implémente.
- **`application/use_cases/`** : orchestration. Un use case = une intention utilisateur.
- **`infrastructure/network/`** : cœur de la Partie 4, structuré dès le départ autour de
  la contrainte technique (proxy + inject + import).
- **`infrastructure/providers/`** : chaque plateforme isolée, le cœur ne dépend d'aucune.
- **`ipc/`** : la seule couche qui connaît Tauri. Fine, sans logique.

---

## Partie 3 — Core Rust (détail des modules)

### 3.1 Domain

- **Entities** (identité + cycle de vie) :
  - `Download` : id, source, options choisies, task associée.
  - `Task` : id, état (`Queued | Running | Paused | Completed | Failed | Cancelled`),
    progression, timestamps. **Machine à états** avec transitions validées dans le domaine
    (ex. `Paused → Running` OK, `Completed → Running` interdit).
  - `MediaSource` : URL, type (`ProgressiveMp4 | WebM | Mov | Hls | Dash`), liste de
    `Quality`, métadonnées.
  - `Playlist` : collection ordonnée de `MediaSource`.
- **Value Objects** (immuables, égalité par valeur) :
  - `Quality { height, bitrate, codec, has_audio }`
  - `MediaFormat`, `Progress { percent, speed, eta }`, `MediaUrl` (validé à la construction).
- **Errors** : `DomainError` (ex. `InvalidStateTransition`, `UnsupportedFormat`).
- **Services** : fonctions pures multi-entités (ex. `select_best_quality(constraints)`).

Pas d'`async` ni d'I/O dans `domain`. Ça garantit des tests unitaires triviaux.

### 3.2 Infrastructure (les 8 modules demandés)

1. **IPC** → dans `ipc/` (voir Partie 7). Techniquement présentation, pas infra.
2. **Database** → `SqlxDownloadRepository`, pool `SqlitePool`, migrations SQLx.
3. **Download Engine** → `YtDlpEngine` : spawn `yt-dlp` via `tokio::process::Command`,
   parse stdout, expose une `Stream<Progress>`.
4. **Provider Engine** → registry + résolution du provider adapté à une URL (Partie 6).
5. **Plugin Loader** → chargement des providers (voir note ci-dessous sur « dynamique »).
6. **Network Monitor** → proxy MITM + hook JS + probe (Partie 4).
7. **Event Bus** → `tokio::sync::broadcast::Sender<DomainEvent>`.
8. **Task Scheduler** → sémaphore Tokio pour limiter la concurrence + boucle de dispatch.

> **Note « plugins dynamiques » :** en Rust, le vrai chargement dynamique (`.dll`/`.so`
> via `libloading`) est fragile (ABI instable) et **injustifié en solo**. Recommandation :
> **providers compilés statiquement**, enregistrés dynamiquement **au runtime** dans un
> registry (pattern registry, pas dylib). On garde le mot « dynamique » au sens
> « résolution au runtime », pas « chargement de binaire externe ». Voir Partie 6 et 12.

### 3.3 Configuration, Logging, Settings

- **Settings** : struct sérialisée en TOML/JSON dans le dossier de config Tauri
  (`app_config_dir`). Chargée au boot, exposée en lecture au front via une command.
  Champs : dossier de téléchargement, concurrence max, limite de débit, activation proxy,
  providers actifs, qualité par défaut.
- **Logging** : `tracing` + `tracing-subscriber`, sortie fichier rotatif
  (`tracing-appender`) dans `app_log_dir` + console en dev. Niveau réglable via settings.
- **Config runtime** vs **settings utilisateur** séparés : la config technique (chemins
  binaires yt-dlp/ffmpeg) est détectée/validée au boot ; les préférences sont éditables.

---

## Partie 4 — Analyse réseau (mécanismes de détection)

> **Révision 2026-07-19 :** le **proxy MITM (ancien mécanisme A) est ABANDONNÉ**
> (voir 4.5). Il est remplacé, pour la détection live pendant la navigation, par une
> **extension Chromium sur Opera GX** qui pousse ses détections vers un **serveur
> WebSocket local** dans l'app (Partie 4.6). Les autres mécanismes (injection JS
> interne, import manuel, probe direct) restent inchangés. Toutes les sources
> convergent vers le **même pipeline** : `probe` → `MediaDetected` → UI.

### 4.1 Les mécanismes de détection

**Mécanisme A — Extension Opera GX + WebSocket local (principal, live).** — voir 4.6.
Une extension Manifest V3 observe les requêtes de l'onglet actif via
`chrome.webRequest.onBeforeRequest` (mode observation, pleinement supporté en MV3),
pré-filtre par la regex média, et envoie `{ mediaUrl, tabUrl, tabTitle }` à un serveur
WebSocket local (`127.0.0.1`, avec token) dans l'app Tauri. Le backend réinjecte ça
dans le pipeline `probe` → `MediaDetected`.

- **Avantage décisif** : `webRequest` opère à la **couche réseau**, au-dessus des
  frontières frame/origine. Il voit la requête du manifeste **même dans une iframe
  cross-origin et même depuis un Worker** — ce qui élimine par construction les
  problèmes d'iframe/Worker/CSP rencontrés avec l'injection JS (cas vidmoly).
- **Bonus** : accès natif à `tab.title` → nommage du fichier par le titre de la page,
  proprement, sans scraping fragile.
- **Distribution** : extension chargée en mode développeur (décompressée) dans Opera GX,
  jamais publiée sur un store — cohérent avec la contrainte « perso, local, non distribué ».

**Mécanisme B — Injection de script dans la fenêtre d'extraction interne.**
Un script (`inject/hook.js`) injecté via `WebviewWindowBuilder::initialization_script`
dans les **fenêtres d'extraction de l'app** (pas dans Opera GX — on ne contrôle pas ses
pages). Monkey-patch `window.fetch`/`XMLHttpRequest` + interception `Worker` + relais
`postMessage` iframe→top. **Statut : implémenté et fonctionnel.** Reste utile pour
l'extraction ponctuelle depuis une URL de page sans l'extension.

**Mécanisme C — Import manuel (fallback garanti).**
Champ de saisie d'URL dans l'UI. Fonctionne toujours. Filet de sécurité universel.

**Mécanisme D — Probe direct d'un lien média.**
Si l'URL collée est déjà un lien direct `.mp4`/`.m3u8`/`.mpd`, on la classe/parse
directement sans extraction. **Statut : implémenté.**

### 4.2 Analyse HTTP & détection des types

Pour chaque URL candidate (issue de A, B ou C) :

1. **HEAD** (ou GET Range `bytes=0-1`) via **`reqwest`** → récupère `Content-Type`,
   `Content-Length`, `Accept-Ranges`.
2. Classification :
   - `video/mp4`, `.mp4`, `video/webm`, `.webm`, `video/quicktime`/`.mov` →
     **média progressif** direct.
   - `application/vnd.apple.mpegurl`, `application/x-mpegURL`, `.m3u8` → **HLS**.
   - `application/dash+xml`, `.mpd` → **DASH**.
3. Émission d'un `MediaSource` provisoire, enrichi par le parsing ci-dessous.

### 4.3 Parsing des manifestes

- **HLS (M3U8)** :
  - Crate recommandée : **`m3u8-rs`** (parse master + media playlists).
  - Master playlist → liste des `EXT-X-STREAM-INF` = variantes de qualité
    (résolution, bandwidth, codecs). Media playlist → segments.
  - Détecter `EXT-X-KEY` : si `METHOD` ≠ `NONE` **et** clé non triviale → flux
    potentiellement chiffré. On **ne contourne pas le chiffrement** — on marque la source
    comme « protégée » et on laisse yt-dlp gérer si légitime, sinon on informe l'UI.
- **DASH (MPD) — non chiffré uniquement** :
  - Parsing XML via **`quick-xml`** (+ `serde` avec `serde-xml-rs`/`quick-xml` serde).
    Pas de crate DASH mûre en Rust → parsing ciblé maison des `AdaptationSet` /
    `Representation` (width, height, bandwidth, mimeType, codecs).
  - Détection `ContentProtection` dans le MPD → si présent (Widevine/PlayReady) → source
    marquée **protégée**, exclue du téléchargement automatique.
- **Métadonnées & qualités** : pour les manifestes, les qualités viennent des variantes.
  Pour le progressif, on peut estimer via `Content-Length` + durée (si connue) ; sinon on
  délègue l'extraction fine à yt-dlp (`--dump-json`) au moment de l'analyse provider.

### 4.4 Dégradabilité

La détection est **dégradable** — l'app fonctionne même sans l'extension : la fenêtre
d'extraction interne (B), l'import manuel (C) et le probe direct (D) prennent le relais.
L'extension (A) est le confort "live pendant la navigation", pas une dépendance dure.
Cible : Windows / WebView2 (app) + Opera GX / Chromium (extension).

### 4.5 Décisions Partie 4 (révisées 2026-07-19)

| Décision | Justification |
|---|---|
| **Extension Chromium + WS local** = détection live principale | API `webRequest` faite pour ça ; voit iframe/Worker ; zéro complexité TLS ; accès au titre d'onglet |
| **WebSocket local** (pas native messaging) | Découplé, simple à débugger, survit aux reloads ; bind `127.0.0.1` + token |
| **Proxy MITM ABANDONNÉ** (`hudsucker`/`rcgen` retirés) | Surdimensionné (tout le système) ; friction TLS/certs ; l'extension couvre l'usage réel (un navigateur ciblé) pour un coût bien moindre |
| Injection JS interne conservée | Utile pour l'extraction ponctuelle hors extension ; déjà implémentée |
| `m3u8-rs` + `quick-xml` | Parsing manifestes éprouvé / suffisant |
| **Aucun contournement de DRM/chiffrement** | Hors périmètre ; on détecte et on signale |

### 4.6 Architecture de la détection live (extension → WS → pipeline)

```
Opera GX (onglet actif)
   │  chrome.webRequest.onBeforeRequest (observation, MV3)
   ▼
Extension MV3 (service worker)
   │  pré-filtre regex média ; { mediaUrl, tabUrl, tabTitle }
   │  ws://127.0.0.1:<port>  (+ token dans le handshake)
   ▼
App Tauri — serveur WS local (infrastructure : ExtensionWsBridge)
   │  émet DomainEvent::MediaDetected via un producteur de plus sur l'event bus
   ▼
[ pipeline EXISTANT, inchangé ]  probe → MediaSource → IPC event → UI
```

**Cohérence avec l'archi existante** : le pont WS est **un adaptateur d'infrastructure de
plus**, exactement au même titre que `HttpMediaProbe` ou le scheduler. Il ne fait
qu'alimenter le bus d'events (`MediaDetected`) — **zéro changement au domaine, à
l'application, au contrat IPC, à Zustand/TanStack Query**. C'est la concrétisation propre
de ce que le proxy était censé faire, en plus simple.

**Sécurité (proportionnée au local)** : bind `127.0.0.1` uniquement + token partagé dans
le handshake (empêche une page localhost random de spammer). Le pont **n'a aucun pouvoir
d'action** : il ne peut que proposer des candidats au probe ; **un téléchargement reste
toujours un clic utilisateur**. Au pire un client malveillant provoque du trafic de probe,
jamais un téléchargement non voulu.

**Piège MV3 à anticiper** : le service worker est éphémère (le navigateur peut le tuer au
repos) → la connexion WS doit se **reconnecter automatiquement** au réveil, et l'app doit
tolérer déconnexions/reconnexions sans état cassé.

**Composant hors app Tauri** : l'extension est le premier morceau du système qui vit
*en dehors* de l'app. C'est une expansion délibérée et assumée de la frontière système,
justifiée par l'usage. Elle reste locale, non distribuée, chargée à la main.

---

## Partie 5 — Pipeline de téléchargement

### 5.1 Workflow complet

```
Détection (Partie 4)
   → Analyse (probe + parsing + provider.extract_metadata)
   → Création Task en SQLite (état Queued, options figées)
   → File d'attente (TaskScheduler)
   → Acquisition d'un permis (sémaphore Tokio = concurrence max)
   → Lancement yt-dlp (tokio::process, args construits par le provider)
   → Lecture stdout/stderr ligne par ligne (async)
   → Parsing progression (%/vitesse/ETA) → update Task + event IPC (throttlé)
   → (si flux séparés audio/vidéo) merge FFmpeg
   → Vérification fichier de sortie
   → État Completed → insertion History → event IPC final
```

### 5.2 Gestion des états et de la concurrence

- **Scheduler** : boucle qui lit les tâches `Queued`, borne le parallélisme par un
  `tokio::sync::Semaphore` (valeur = concurrence max des settings). Chaque tâche tourne
  dans sa propre `tokio::task`.
- **Progression** : le parser lit les lignes yt-dlp (`--newline`, format
  `--progress-template` pour un output stable et parsable) → structure `Progress`.
  Les events IPC sont **throttlés** (ex. max 4/s par tâche) pour ne pas noyer le front.

### 5.3 Pause / Reprise / Annulation

- **Annulation** : on garde le `Child` handle ; `child.kill()` + état `Cancelled`.
  Fichiers partiels nettoyés selon settings.
- **Pause/Reprise** : yt-dlp ne gère pas un « pause » propre. Stratégie réaliste :
  - Pause = kill du process en **conservant les fichiers `.part`**.
  - Reprise = relance yt-dlp avec `--continue` (reprend le `.part` via HTTP Range si le
    serveur supporte `Accept-Ranges`).
  - **Limite** : la reprise n'est pas garantie pour tous les flux (HLS segmenté reprend
    au niveau segment ; serveurs sans Range → redémarrage). L'UI le reflète.
- **Reprise après crash de l'app** : au boot, les tâches `Running` en base sont remises en
  `Queued` (ou reprises via `--continue`). La base est la source de vérité.

### 5.4 Débit, parallélisme, merge

- **Limitation de débit** : `yt-dlp --limit-rate` (global via settings, réparti ou par
  tâche selon choix — recommandation : par tâche, plus simple).
- **Parallélisme** : concurrence inter-tâches via sémaphore ; intra-tâche laissé à yt-dlp
  (`--concurrent-fragments` pour HLS/DASH).
- **Merge FFmpeg** : délégué à yt-dlp (`--merge-output-format mp4`) qui appelle FFmpeg
  lui-même. On ne réimplémente pas le merge. On fournit juste le chemin FFmpeg à yt-dlp.

### 5.5 Gestion des erreurs

- Erreurs classées : `NetworkError`, `YtDlpError(code, stderr)`, `MergeError`,
  `DiskError`, `Cancelled`. Stockées sur la Task (message + code).
- **Retry** : politique simple (N essais, backoff) sur erreurs réseau transitoires
  uniquement ; pas de retry sur erreur de format/DRM.

### 5.6 Décisions Partie 5

| Décision | Justification | Compromis |
|---|---|---|
| yt-dlp fait tout le travail lourd + merge | Ne pas réinventer l'extraction/merge | Dépendance à un binaire externe (versionné, vérifié au boot) |
| Pause = kill + `--continue` | yt-dlp n'a pas de vraie pause | Reprise non garantie selon serveur/flux |
| SQLite = source de vérité pour reprise | Robustesse post-crash | — |
| `--progress-template` | Output parsable stable | Dépend du format yt-dlp (à figer/tester) |

---

## Partie 6 — Providers

### 6.1 Le trait `Provider`

```
trait Provider (esquisse conceptuelle, pas du code final) :
  - id() -> &str
  - matches(url) -> bool                     # détection de site
  - extract_metadata(url) -> MediaSource     # via yt-dlp --dump-json le plus souvent
  - available_qualities(...) -> Vec<Quality>
  - build_ytdlp_args(source, options) -> Vec<String>
  - capabilities() -> Capabilities           # {playlist, live, subtitles, chapters...}
```

### 6.2 Providers réellement utilisés

**Périmètre validé :** `generic`, `youtube`, `reddit`, `vimeo`.
`twitter`, `instagram`, `facebook`, `tiktok` sont **retirés du périmètre initial** ;
ils pourront être ajoutés plus tard via le registry sans toucher au cœur.
`generic` est le **fallback universel** (délègue tout à yt-dlp sans logique spécifique).
La plupart des providers spécifiques ne font que : (a) `matches` par domaine,
(b) déclarer des capacités, (c) ajuster quelques args yt-dlp. **Le gros du travail reste
yt-dlp** ; les providers ne réimplémentent pas l'extraction.

### 6.3 Enregistrement « dynamique » (au runtime, statiquement compilé)

- Un `ProviderRegistry` construit au boot enregistre toutes les instances.
- Résolution : on itère et on prend le premier `matches(url)` vrai, sinon `generic`.
- **Le cœur ne dépend jamais d'un provider concret** : il ne connaît que le trait
  `Provider` et le registry. Ajouter un provider = ajouter un fichier + 1 ligne
  d'enregistrement. Pas de dylib (voir Partie 3.2 et 12).

### 6.4 Décision Partie 6

| Décision | Justification |
|---|---|
| Trait `Provider` + registry runtime | Extensible, cœur découplé |
| Providers statiques (pas de dylib) | ABI Rust instable ; solo → inutile et fragile |
| `generic` délègue à yt-dlp | Couvre 90% des cas sans code |

---

## Partie 7 — IPC

### 7.1 Principes

- **Commands** = requêtes front→back (retour typé, `Result<T, AppError>`).
- **Events** = push back→front (progression, détection, notifications).
- **Types TS générés** depuis Rust via **`ts-rs`** (dérive `#[derive(TS)]` sur les DTO,
  génère `ipc.gen.ts`). Une seule source de vérité pour les types.
- **Erreurs** : `AppError` sérialisable (`{ kind, message }`), mappée depuis les erreurs
  domain/infra. Jamais de `unwrap` traversant l'IPC.

### 7.2 Streaming de progression

Le backend émet des events `download://progress` (payload `{ taskId, percent, speed, eta }`),
throttlés. Le front écoute via `listen` et met à jour le store/Query cache.

### 7.3 Exemple Rust (esquisse, non définitif)

```rust
#[derive(serde::Serialize, ts_rs::TS)]
#[ts(export)]
pub struct ProbeResult { pub sources: Vec<MediaSourceDto> }

#[tauri::command]
async fn probe_url(state: tauri::State<'_, AppState>, url: String)
    -> Result<ProbeResult, AppError> {
    state.use_cases.probe_url.execute(url).await.map_err(AppError::from)
}
```

### 7.4 Exemple React (esquisse)

```tsx
const { data } = useQuery({
  queryKey: ['probe', url],
  queryFn: () => invoke<ProbeResult>('probe_url', { url }),
});

useEffect(() => {
  const un = listen<Progress>('download://progress', (e) => updateProgress(e.payload));
  return () => { un.then(f => f()); };
}, []);
```

### 7.5 Décision Partie 7

| Décision | Justification |
|---|---|
| `ts-rs` pour les types | Zéro désync front/back, généré au build |
| Erreurs typées sérialisables | Front gère les cas proprement |
| Events throttlés | Perf UI |

---

## Partie 8 — Frontend

### 8.1 Organisation feature-first

Chaque feature (`detection`, `downloads`, `queue`, `history`, `settings`) contient :

```
features/downloads/
├── components/     # UI spécifique à la feature
├── hooks/          # useDownloads, useStartDownload (wrappe TanStack Query)
├── api/            # appels invoke() typés (ipc.gen.ts)
├── store/          # état UI local à la feature (Zustand slice) si besoin
└── types.ts        # types dérivés / view models
```

### 8.2 Séparation des responsabilités

- **UI** (`components/`) : présentation pure, props in / events out.
- **Business/API** (`hooks/` + `api/`) : orchestration, appels IPC via TanStack Query
  (cache, invalidation, retry). Les mutations (`startDownload`, `cancel`) invalident les
  queries pertinentes.
- **Stores Zustand** : uniquement l'état **UI transverse** (filtre courant, sélection,
  panneau ouvert). **Pas** l'état serveur → ça, c'est TanStack Query.
- **Services/lib** : client IPC, query keys, formatters.
- **Types** : `ipc.gen.ts` (généré) + view models locaux.
- **Router** : React Router, une route par vue principale (Détection, Téléchargements,
  File, Historique, Réglages).

### 8.3 Règle d'or

**TanStack Query = état serveur (backend Rust). Zustand = état UI.** Ne jamais dupliquer
l'état backend dans Zustand. Les events IPC mettent à jour le cache Query directement.

---

## Partie 9 — Base SQLite

### 9.1 Schéma (tables)

- **downloads** : `id`, `source_url`, `provider_id`, `title`, `format`, `output_path`,
  `created_at`. (Le « quoi ».)
- **tasks** : `id`, `download_id (FK)`, `state`, `progress`, `speed`, `eta`,
  `error_code`, `error_message`, `started_at`, `updated_at`. (Le « où ça en est ».)
- **queue** : `task_id (FK)`, `priority`, `position`, `enqueued_at`. (Ordre d'exécution.)
- **providers** : `id`, `enabled`, `config_json`. (Activation/paramétrage.)
- **history** : `id`, `download_id (FK)`, `completed_at`, `final_path`, `duration_ms`,
  `bytes`. (Archive des terminés.)
- **settings** : `key`, `value` (clé/valeur) — ou une ligne unique JSON.
- **playlists** : `id`, `title`, `source_url`, `created_at`.
- **playlist_items** : `playlist_id (FK)`, `download_id (FK)`, `position`.
- **logs** : `id`, `level`, `message`, `context_json`, `created_at` (logs applicatifs
  consultables dans l'UI ; les logs détaillés restent dans les fichiers `tracing`).

### 9.2 Relations

- `downloads 1 — 1 tasks` (une tâche d'exécution par download).
- `tasks 1 — 1 queue` (entrée de file par tâche active).
- `downloads 1 — 0..1 history` (rempli à la complétion).
- `playlists 1 — N playlist_items N — 1 downloads`.

### 9.3 Index & justification

| Index | Colonne(s) | Pourquoi |
|---|---|---|
| `idx_tasks_state` | `tasks.state` | Le scheduler filtre par état (`Queued`, `Running`) en boucle |
| `idx_queue_position` | `queue.priority, position` | Tri de la file à chaque dispatch |
| `idx_downloads_provider` | `downloads.provider_id` | Filtres UI par provider |
| `idx_history_completed` | `history.completed_at` | Tri chronologique de l'historique |
| FK indexées | toutes les FK | Jointures fréquentes downloads↔tasks↔history |

Migrations gérées par **SQLx migrations** (`sqlx::migrate!`), fichiers `.sql` versionnés.
Mode **WAL** activé pour de meilleures écritures concurrentes.

---

## Partie 10 — Tests (stratégie allégée, solo)

- **Unitaires — Domain** (priorité #1, ROI max) : machine à états des tâches, sélection de
  qualité, validation d'URL, value objects. Pas de mocks (code pur).
- **Intégration — cas critiques uniquement** :
  - Parsing manifestes : jeux de fichiers M3U8/MPD réels en fixtures → vérifier les
    qualités extraites et la détection « protégé ».
  - Pipeline download : yt-dlp **mocké** (trait `DownloadEngine` avec impl fake émettant
    une séquence de `Progress`) → vérifier transitions d'état + events + persistance.
  - Repositories SQLx : contre une SQLite en mémoire, migrations appliquées.
- **IPC — smoke tests** : quelques commands appelées de bout en bout (Tauri en mode test).
- **Volontairement exclus** : E2E complets, benchmarks, couverture exhaustive. À
  reconsidérer seulement si une régression récurrente le justifie.

Le Repository Pattern et les traits de la couche `application` rendent tout ça
mockable sans effort — c'est la vraie raison de les avoir.

---

## Partie 11 — Roadmap (phases indépendantes, sans pression)

1. **Phase 0 — Architecture** : validation de cet ADR (← on est ici).
2. **Phase 1 — Core Rust** : domain + application (ports/use cases) + squelette infra,
   AppState/composition root. Compile, testable, sans UI.
3. **Phase 2 — IPC** : commands/events de base + génération `ts-rs`.
4. **Phase 3 — SQLite** : migrations, repos, reprise après crash.
5. **Phase 4 — yt-dlp** : engine, parsing progression, pause/annulation, scheduler.
6. **Phase 5 — Frontend** : shell UI, TanStack Query, vues Téléchargements/File/Historique.
7. **Phase 6 — Détection (Partie 4)** : import manuel + probe + parsing manifestes, puis
   injection JS (fenêtre d'extraction interne). ✅ **Fait et validé en réel.**
8. **Phase 7 — Providers** : `generic` d'abord, puis youtube/reddit/… au besoin réel. ✅
9. **Phase 8 — Améliorations** : playlists, limitation de débit fine, sous-titres, etc.
   (débit, sous-titres, settings, historique, annulation/pause réelles : ✅ ; reste playlists)
10. **Phase 9 — Modes de fenêtre** (Partie 1.8) : bascule standard/volatile en live,
    layout compact, persistance dans Settings. *Frontend + réglages fenêtre, pas de
    changement domaine/IPC.*
11. **Phase 10 — Détection live** (Partie 4.6) : extension Opera GX MV3 + serveur WS local
    (`ExtensionWsBridge`) + intégration au pipeline `MediaDetected`. *Remplace l'ancienne
    « Phase 9 proxy MITM », abandonnée.*

Chaque phase est **livrable et utilisable seule**. Les Phases 9 et 10 sont indépendantes
l'une de l'autre (on peut faire le mode volatile sans l'extension, et inversement), mais
leur combinaison est le vrai objectif produit (widget flottant qui se peuple tout seul).

---

## Partie 12 — Bonnes pratiques : décisions structurantes

| Décision | Pourquoi retenue | Alternatives écartées & compromis | Limites |
|---|---|---|---|
| Clean Archi 3 couches | Testabilité + clarté sans boilerplate | Hexagonal strict (trop de traits), monolithe plat (non testable) | Discipline manuelle des dépendances (pas de lint qui l'impose) |
| DI manuelle (AppState + Arc<dyn>) | Idiomatique Rust, lisible | Framework DI → magie inutile en solo | Câblage explicite au boot |
| yt-dlp/FFmpeg externes | Robustesse d'extraction, maintenus par la communauté | Réimplémentation Rust → impossible/inutile | Dépendance binaire à versionner/valider au boot |
| Providers statiques + registry | Extensible, cœur découplé | Dylib (`libloading`) → ABI fragile | Recompilation pour ajouter un provider |
| Détection : extension + inject + import (proxy abandonné) | Extension = bon outil pour un navigateur ciblé, voit iframe/Worker, pas de TLS | Proxy MITM (surdimensionné, friction certs) ; méthode unique (fragile) | Extension à charger à la main dans Opera GX ; SW MV3 éphémère (reconnexion WS) |
| SQLite source de vérité | Reprise post-crash, front jetable | État en mémoire → perte de données | — |
| TanStack Query (serveur) + Zustand (UI) | Séparation claire, moins de bugs d'état | Tout en Zustand → resync manuelle pénible | — |
| `ts-rs` types générés | Zéro désync IPC | Types dupliqués à la main → dérive | Build step supplémentaire |
| Pas de CQRS/ES/télémétrie/multi-user | Hors périmètre perso/local | — | — |
| Aucun contournement DRM | Hors périmètre ; on détecte/signale | — | Flux protégés non téléchargeables auto |

---

## Décisions validées (2026-07-18)

1. **Proxy MITM** : **différé**. Phase 1 et suivantes s'appuient sur import manuel +
   injection JS. Le proxy (`hudsucker` + `rcgen`) sera ajouté dans une phase ultérieure,
   une fois le pipeline stable. La couche `network` est structurée pour l'accueillir sans
   refonte (le proxy est un simple producteur d'events `MediaDetected` de plus).
2. **Providers** : périmètre initial limité à **`generic`, `youtube`, `reddit`, `vimeo`**.
   Les autres sont retirés ; ajout ultérieur possible via le registry.
3. **Binaires** : **sidecar Tauri** (yt-dlp + FFmpeg embarqués). L'app est autonome, ne
   dépend pas du PATH système. Résolution du chemin sidecar au boot + validation de version.
4. **Pause** : **kill + `--continue`** validé (simple et robuste, reprise best-effort).
5. **Monorepo** : structure **`apps/frontend` + `apps/src-tauri`** validée, appliquée.

### Impact sur la roadmap

- Phase 6 (détection) : commencer par **import manuel + probe + parsing manifestes**, puis
  **injection JS**. ✅ Fait.
- Phase 7 (providers) : n'écrire que `generic`, `youtube`, `reddit`, `vimeo`. ✅ Fait.

## Décisions validées (2026-07-19) — révision widget + détection live

6. **Modes de fenêtre** : une seule fenêtre à état dynamique, bascule standard/volatile
   en live (Partie 1.8). Purement présentation ; persistance du mode + géométrie dans les
   `Settings` existants.
7. **Détection live** : **extension Chromium MV3 sur Opera GX** comme mécanisme principal
   (Partie 4.6).
8. **Canal extension ↔ app** : **WebSocket local** (`127.0.0.1` + token), pas native
   messaging.
9. **Proxy MITM** : **ABANDONNÉ**. `hudsucker`/`rcgen` retirés de la roadmap. L'extension
   couvre l'usage réel (un navigateur ciblé) pour un coût bien moindre et sans friction TLS.
10. **Répartition des modes** : volatile = mode principal en navigation (se peuple via
    l'extension) ; standard = gestion (historique, réglages, analyse manuelle, sélection
    fine de qualité).
```
