# État du projet — Video DL

_Dernière mise à jour : session de développement autonome._

## Résumé

**MSVC installé** (côté utilisateur) et **shell Tauri assemblé et lancé avec succès**.
`cargo tauri dev` compile, `video-dl.exe` tourne, la fenêtre "Video DL" est stable
(60 s+ sans crash, aucune erreur dans le journal d'événements Windows), et le backend
bootstrap réellement (SQLite créée + migrations appliquées dans
`%APPDATA%\com.local.videodl\video-dl.sqlite`). Backend Rust **complet et testé**
(76 tests + 2 e2e réels ignorés, clippy clean), frontend React **complet, typecheck +
build OK**, rendu identique confirmé via le serveur Vite (mêmes DOM/CSS que la
fenêtre Tauri en mode dev).

**Bug corrigé (2026-07-18, remonté par l'utilisateur en test réel) :** boucle de
re-render infinie ("Maximum update depth exceeded") sur la page Téléchargements.
Cause : `useDownloadsStore((s) => Object.values(s.items))` allouait un nouveau
tableau à chaque appel ; `useSyncExternalStore` (sous Zustand) compare par
référence stricte et voyait donc un changement à chaque vérification → boucle.
Correctif : `useShallow` de `zustand/react/shallow` pour comparer par contenu.
Un `ErrorBoundary` (`apps/frontend/src/app/ErrorBoundary.tsx`) a aussi été ajouté
autour de l'`<Outlet/>` du layout (keyé par pathname) pour qu'un futur crash de
page reste local, avec boutons Réessayer / Recharger, au lieu d'un écran blanc.
Outillage de test frontend ajouté (Vitest + Testing Library) avec un test de
régression qui reproduit exactement l'erreur si le bug revient (vérifié en
réintroduisant le bug : le test échoue avec le même message) — `npm test` dans
`apps/frontend`.

**Nouvelle fonctionnalité (2026-07-18, sur demande utilisateur, accélération de la
Phase 6) : extraction depuis une page web.** Pour les URLs de pages classiques (pas
un lien direct vers un média — ex. sites d'agrégation type anime-sama), le pipeline
détection ne pouvait rien faire (`probe failed: unrecognized media type`). Ajout :

- Port `PageMediaExtractor` (`application/ports`) : `extract_silent` (fenêtre
  cachée, timeout fixe, résultat en lot) et `extract_interactive` (fenêtre
  visible, résultats en flux via un channel tokio, se termine à la fermeture
  de la fenêtre par l'utilisateur).
- Implémentation Tauri (`desktop/extraction.rs`) : `TauriPageExtractor` +
  `ExtractorRegistry` (map label de fenêtre → sender), script injecté
  (`infrastructure/network/inject/hook.js`) qui hook `fetch`/`XMLHttpRequest` +
  scan des éléments `<video>/<source>`, filtre par extension, dédoublonne côté
  JS, remonte via la commande `report_detected_url`.
- Use case `ExtractFromPage` : réutilise le `MediaProbe` existant pour chaque
  URL candidate (même pipeline que les liens directs — classify/HLS/DASH/DRM),
  dédoublonne, publie `MediaDetected` par trouvaille.
- `DomainEvent::MediaDetected` enrichi : porte désormais le `MediaSource`
  complet (pas juste l'URL), pour que le flux interactif n'ait pas besoin d'un
  aller-retour supplémentaire.
- IPC : `extract_from_page` (silencieux, appelé automatiquement en fallback
  après un échec `probe_url` de type "probe") et `open_interactive_extraction`
  (fenêtre visible, déclenchée par un bouton si le passage silencieux ne
  trouve rien).
- Frontend : `DetectionPage` gère désormais un tableau de sources (plusieurs
  flux possibles sur une page), écoute `media://detected` pendant une session
  interactive pour afficher les trouvailles en direct, dédoublonne par URL.
- Tests : `tests/page_extraction.rs` (extracteur factice — silencieux,
  interactif, dédoublonnage, tolérance aux échecs de probe individuels) +
  `DetectionPage.test.tsx` (8 tests : lien direct, fallback automatique,
  pas de fallback sur erreur de validation, proposition d'extraction
  interactive, flux live sans doublon). Backend 79 tests, clippy clean,
  frontend build + 8 tests verts.
- **Hors périmètre assumé** (voir échange de conception) : login/paywall, DRM
  (toujours marqué "protégé", jamais contourné), sites à forte obfuscation —
  l'import manuel/direct reste le filet de secours dans tous les cas.
- **Diagnostic réel effectué (2026-07-18)** sur le cas anime-sama remonté par
  l'utilisateur : le chemin interactif s'est ouvert correctement, mais aucun
  flux n'était détecté après clic sur Lecture. Investigation directe (hors
  Tauri, via le navigateur intégré) sur `vidmoly.biz` (l'hébergeur derrière
  l'iframe du lecteur) : la vidéo est chargée via **MSE + Web Worker**
  (aucune requête `.m3u8` visible au niveau page — vidéo servie en `blob:`).
  Un Worker a son propre contexte global (`self.fetch`/`XHR` séparés de
  `window`), invisible au hook initial. Confirmé via la doc officielle
  WebView2 que `AddScriptToExecuteOnDocumentCreated` couvre bien les iframes
  cross-origin (donc ce n'était pas le problème) — uniquement les Workers.
- **Correctif ajouté** : `hook.js` intercepte désormais `new Worker(...)`
  (workers classiques uniquement, pas `{type:'module'}`), enveloppe le script
  réel dans un shim chargé via `importScripts` qui patch `self.fetch`/XHR à
  l'intérieur du worker, et remonte les URLs interceptées par `postMessage`.
  Repli sûr sur un worker non-modifié si l'enveloppe échoue (site cassé plutôt
  que silencieusement non fonctionnel évité). **Vérifié avec un harnais de
  test isolé** (page HTML servie par Vite, reproduisant exactement le motif
  vidmoly : fetch interne à un worker) — la technique capte bien l'URL et le
  worker continue de fonctionner normalement.
- **Limite connue restante** : si le script du worker est chargé depuis un
  domaine tiers avec CORS restrictif, `importScripts` échoue et on retombe
  silencieusement sur le worker non intercepté (pas de détection, mais site
  non cassé). Toujours pas de contournement DRM/EME.
- **Vraie cause trouvée (2026-07-18, via logs de diagnostic dans les DevTools
  WebView2)** : ce n'était PAS un problème de Worker. Le manifeste
  (`prx-*.vmwesa.online/hls2/.../master.m3u8`) est bien chargé sur le thread
  principal de l'iframe vidmoly.biz et **était déjà détecté** par le hook
  fetch/XHR de base. Le vrai bug : `window.__TAURI__` (le pont IPC de Tauri)
  n'existe que dans la fenêtre de premier niveau, **pas** dans les iframes
  cross-origin — donc l'URL trouvée ne pouvait jamais appeler `invoke()`
  depuis l'intérieur de l'iframe.
- **Correctif (partiel)** : les frames imbriquées sans pont IPC relaient l'URL
  détectée à la fenêtre de premier niveau via `window.top.postMessage(...)`.
- **Fausse piste explorée et écartée** : ACL/capacités Tauri. J'ai ajouté
  l'enregistrement des commandes dans `build.rs`
  (`Attributes::app_manifest(AppManifest::new().commands(&[...]))`) et deux
  fichiers `capabilities/*.json` (main = accès complet, `extract-*` = accès
  minimal à `report_detected_url` uniquement, pour tout domaine http/https).
  Cette partie reste une bonne pratique de sécurité à garder, mais **ce
  n'était pas la cause du bug** : même avec les capacités correctement
  configurées, `window.__TAURI__` restait introuvable même après 2s de retry.
- **Vraie cause finale (confirmée dans le code source de tauri-utils
  2.9.3, `config.rs`)** : `with_global_tauri: false` est la valeur par
  défaut de Tauri v2. Sans ce flag, l'objet global `window.__TAURI__`
  **n'est jamais injecté, dans aucune fenêtre**. Le frontend principal
  fonctionnait quand même car il importe le module npm `@tauri-apps/api`
  (qui n'a pas besoin du global) — mais mon script injecté brut (`hook.js`,
  qui ne peut pas faire d'`import`) n'avait que ce global comme point
  d'accès à l'IPC, d'où l'échec systématique, y compris sur la fenêtre
  principale de l'extraction (pas seulement les iframes).
- **Correctif final** : `"app": {"withGlobalTauri": true}` ajouté à
  `tauri.conf.json`. Rebuild automatique en cours de vérification.
- **Extraction confirmée fonctionnelle sur le vrai site** (2026-07-19) : deux
  flux HLS détectés sur anime-sama (master + une piste), qualités affichées,
  téléchargement réellement démarré par yt-dlp (vrais octets vidéo H.264
  écrits sur disque, confirmé par l'utilisateur).
- **Corrections de qualité de vie suite au premier test réel** :
  - Nom de fichier basé sur le **titre de la page d'origine** (capturé via
    `document.title` dans `hook.js`, remonté par `report_detected_url` →
    `ExtractedCandidate.title` → `MediaSource.title` → `Download.title` →
    `EngineRequest.title` → argument `-o` de yt-dlp) au lieu du nom du
    manifeste HLS (qui incluait le token de sécurité signé du CDN).
    Sanitisation dédiée (`sanitize_filename`) pour les caractères interdits
    sous Windows, tests unitaires associés.
  - Fichiers `.part`/`.ytdl` orphelins (interrompus par les rebuilds de dev
    successifs) nettoyés dans le dossier Téléchargements de l'utilisateur.
- **Bug réel trouvé après le fix de nommage** : le téléchargement échouait
  systématiquement et immédiatement (`.ytdl` vide, jamais de `.part`) avec
  en base l'erreur `"reading yt-dlp output: stream did not contain valid
  UTF-8"`. Diagnostiqué en interrogeant directement la vraie base SQLite
  utilisateur (script de test jetable, supprimé après usage) plutôt que de
  deviner. **Cause** : `yt-dlp` (Python) écrit son chemin de destination sur
  stdout dans l'encodage de la console Windows plutôt qu'en UTF-8 — et ce
  chemin contient un accent dans le profil utilisateur (`C:\Users\<Prénom>\Downloads\`), le même piège
  d'encodage rencontré plusieurs fois ailleurs dans ce projet. Ma lecture de
  stdout via `AsyncBufReadExt::lines()` (UTF-8 strict) plantait et tuait le
  process au premier octet invalide. **Correctif double** : (1) variables
  d'environnement `PYTHONUTF8=1`/`PYTHONIOENCODING=utf-8` passées à yt-dlp
  pour forcer un stdout UTF-8 propre à la source ; (2) lecture ligne par
  ligne réécrite en `read_line_lossy()` (décodage tolérant,
  `String::from_utf8_lossy`) comme filet de sécurité — un octet invalide ne
  tue plus jamais un téléchargement, quelle que soit la cause future.
  55 tests backend, clippy clean, re-vérifié avec le test e2e réel.
- **✅ Extraction de page validée de bout en bout par l'utilisateur (2026-07-19)** :
  détection sur anime-sama (page classique, pas de lien direct) → extraction
  interactive → 2 flux HLS détectés avec qualités → téléchargement réel via
  yt-dlp → **fichier `.mp4` final propre de 1,77 Go, nom lisible basé sur le
  titre de la page, lu avec succès dans un lecteur vidéo standard**. La
  fonctionnalité complète (Option 1 de l'ADR, accélérée sur demande
  utilisateur) est fonctionnelle en conditions réelles.

**Nouvelle fonctionnalité (2026-07-19, session autonome — "finis le projet applique
toi") : mode fenêtre standard/volatile + détection live via extension Opera GX.**
Suite à la demande utilisateur d'un widget flottant always-on-top avec détection
automatique pendant la navigation, deux décisions verrouillées via question directe
(canal WebSocket local ; abandon du proxy MITM) puis ADR mis à jour (Parties 1.8,
4, 11, 12 — voir `docs/architecture/ADR-0001-architecture-globale.md`), suivi de
l'implémentation complète en autonomie :

- **Phase 9 — mode fenêtre.** Une seule fenêtre Tauri, deux présentations
  dynamiques (`Settings.window_mode: WindowMode`) : `standard` (UI actuelle,
  routeur complet) et `volatile` (widget compact 380×480, always-on-top, sans
  décorations, positionné coin haut-droit par défaut). Bascule à la volée sans
  redémarrage (`toggleWindowMode()`), géométrie persistée (debounce 600 ms sur
  `onMoved`/`onResized`). `apps/frontend/src/app/Root.tsx` route entre
  `<VolatileLayout/>` et le routeur complet selon `uiStore.mode`. 9 permissions
  `core:window:allow-*` ajoutées à `capabilities/default.json` (leçon retenue de
  `report_detected_url` plus tôt dans le projet : sans capacité explicite, l'IPC
  échoue silencieusement). Testé : `VolatileLayout.test.tsx` (3 tests — état vide,
  flux détection→téléchargement, dédoublonnage par URL).
- **Phase 10 — détection live.** Extension Chromium MV3 (`apps/extension/`,
  jamais publiée sur un store, installation manuelle documentée dans son
  `README.md`) qui observe `chrome.webRequest.onBeforeRequest` sur l'onglet
  actif, pré-filtre par regex média, et pousse `{mediaUrl, tabUrl, tabTitle}` à
  l'app via WebSocket local (reconnexion avec backoff — le service worker MV3
  est éphémère). Côté Rust : `ExtensionWsBridge`
  (`infrastructure/network/extension_ws.rs`, `tokio-tungstenite`) — bind
  **127.0.0.1 uniquement**, token vérifié au handshake (généré au premier
  lancement, affiché en clair dans **Réglages** avec bouton copier pour le
  coller dans la page d'options de l'extension). Les messages reçus convergent
  sur le **même pipeline que toutes les autres sources de détection**
  (nouveau use case partagé `IngestDetection` → `probe` → événement
  `MediaDetected`), donc aucune logique dupliquée. L'extension ne peut que
  *proposer* des URLs candidates ; le téléchargement reste toujours un clic
  explicite dans l'app.
- **Tests** : backend 62 tests (dont 3 nouveaux tests d'intégration
  `tests/extension_ws.rs` — vrai client WebSocket sur vrai port TCP éphémère,
  token correct/incorrect, tolérance à une URL de probe invalide), clippy
  clean. Frontend : build + typecheck OK, 11 tests unitaires verts.
- **Non vérifié** (nécessite l'utilisateur) : comportement réel de l'extension
  chargée dans Opera GX (le contrôle du bureau de l'assistant ne peut pas
  piloter un navigateur externe ni installer une extension non empaquetée) —
  seule la chaîne serveur (bridge WS + pipeline) est testée par du code, pas
  l'observation réseau réelle côté navigateur.

**Miniature/durée dans le widget (2026-07-19)** : ajout d'une prévisualisation
(poster + durée) à côté du bouton télécharger, sur le modèle de Video
DownloadHelper. Source retenue après évaluation avec l'utilisateur : **le
poster de la page** (`<video poster>` / `og:image`), capturé au point de
détection existant (hook.js injecté / content-script de l'extension), pas
d'extraction FFmpeg (coûteuse, FFmpeg non garanti installé) ni de parsing
`#EXT-X-IMAGE` (tag HLS non standard, quasi jamais présent). `MediaSource`
gagne deux champs optionnels (`thumbnail: Option<String>`,
`duration_secs: Option<u32>`), threadés à travers tous les chemins de
détection existants (probe manuel = toujours `None`, extraction de page,
extension) via un petit struct partagé `DetectionMeta` dans
`IngestDetection`. Nouveau `apps/extension/content-script.js` : le service
worker n'a pas accès au DOM, donc un content script lit poster/durée et les
transmet via `chrome.runtime.sendMessage`, mis en cache par onglet dans
`background.js`. Affichage : `<img loading="lazy">` (asynchrone et non
bloquant par nature, aucun état de chargement à gérer côté app) avec repli
sur une icône générique par format si absente ou cassée (`onError`).
64 tests backend (dont 2 nouveaux sur `extension_ws.rs` et
`DetectionMessage`), 13 tests frontend, clippy clean, build OK.

**Widget synchronisé avec la page active (2026-07-19)** : le widget affichait
toutes les détections accumulées depuis le lancement de l'app, même après
avoir changé d'épisode/onglet — évalué avec l'utilisateur avant codage.
Décision : garder tout en historique côté store (rien n'est supprimé), et ne
filtrer que **l'affichage** du widget sur le contexte actif (onglet+page),
via un signal explicite de navigation plutôt qu'une simple absence de
nouvelles détections (seul moyen de gérer correctement le cas "page sans
vidéo"). Extension : nouveaux listeners `chrome.tabs.onActivated`,
`chrome.tabs.onUpdated`, `chrome.webNavigation.onHistoryStateUpdated`
(permission ajoutée) → envoient `{kind:"context", tabId, pageUrl}` sur le
même canal WS que les détections (`{kind:"detection", ...}` maintenant
taggé). Backend : `MediaSource` gagne `source_tab_id`/`page_url` (uniquement
peuplés par la source extension) ; nouvel événement
`DomainEvent::BrowsingContextChanged` → canal IPC
`browsing://context-changed`. Frontend : `detectionsStore` garde
`activeTabId`/`activePageUrl` + un sélecteur `visibleDetections` (détections
sans `sourceTabId` toujours visibles, pour ne pas casser le mode
standard/management). 67 tests backend (dont 5 sur `extension_ws.rs`), 16
tests frontend, clippy clean.

**Détections silencieusement rejetées sur YouTube/sites à hotlink-protection —
corrigé (2026-07-19)** : après le fix de correspondance page/onglet, YouTube
et un site adulte (PimpBunny) ne détectaient toujours rien malgré des logs
d'extension montrant contexte et détection bien envoyés au bon format.
Diagnostic : `HttpMediaProbe` ne posait ni `Referer` ni `Origin` sur sa
requête HTTP — beaucoup de CDN (googlevideo, sites à forte protection
anti-hotlinking) rejettent silencieusement une requête qui ne ressemble pas à
une vraie requête de page, faisant échouer le probe sans indice (`ingest.execute`
retourne juste `None`). `MediaProbe::probe` prend désormais un `referer:
Option<&str>` (dérivé du `page_url` déjà connu via `DetectionMeta`/l'extraction
de page) ; `HttpMediaProbe` pose `Referer` + `Origin` (dérivé) quand connu.
Ajout d'une vérification explicite du status HTTP (auparavant une réponse
d'erreur pouvait glisser jusqu'au classificateur sans message clair). Testé
avec un serveur TCP local capturant la requête brute (`tests/http_probe.rs`,
2 tests). 69 tests backend, clippy clean.

**Gestion historique/file d'attente (2026-07-21)** :
- **Bug réel découvert et corrigé** : le bouton "Retirer" d'un téléchargement
  échoué/annulé ne faisait que vider le store Zustand local — la ligne
  restait en base (`downloads`/`tasks`), donc réapparaissait au prochain
  redémarrage (`list_active` inclut les `failed`, seuls `completed`/
  `cancelled` sont exclus). Nouveau use case `RemoveDownload` (suppression
  réelle en base, restreinte aux états terminaux `failed`/`cancelled` —
  refuse sur une tâche en cours) + commande IPC `remove_download`. Le widget
  et la page Téléchargements l'utilisent maintenant ; bouton "Retirer" ajouté
  aux entrées `cancelled` (absent avant, elles s'accumulaient sans recours).
- **Historique enrichi** : recherche/filtre (titre ou URL), suppression d'une
  entrée (`DeleteHistoryEntry` + `delete_history_entry`, n'efface que la
  ligne d'historique, pas le fichier), "Ouvrir le dossier" (révèle le fichier
  dans l'Explorateur via `explorer.exe /select,`), "Retélécharger" (re-sonde
  l'URL — le format n'est pas stocké en historique, deviner aurait pu envoyer
  un manifeste HLS dans le raccourci HTTP direct réservé aux fichiers
  simples — puis relance le téléchargement).
- 89 tests backend (dont 3 nouveaux sur remove/delete), 25 tests frontend
  (dont 6 nouveaux `HistoryPage.test.tsx`), clippy clean.

**PimpBunny / sites KVS résolu — téléchargeur HTTP direct (2026-07-21)** :
diagnostic complet fait via le navigateur intégré (Claude Browser) avant de
coder — leçon du dernier échec, ne plus deviner. Trouvailles :
- PimpBunny utilise le lecteur **KVS** (`kt_player.js`), très répandu sur les
  sites tube. Les URLs vidéo par qualité (`get_file/.../NNNp.mp4/`) sont
  **directement dans le HTML de la page** (`video_url`/`video_alt_url`),
  aucune requête réseau à sniffer, aucune déobfuscation nécessaire.
- Vérifié via curl : ces URLs redirigent (302) vers un vrai flux
  `video/mp4` complet (66 Mo testé) — accessible avec le bon `Referer`.
- **Mais** ni yt-dlp (contrôle de sécurité sur l'extension `.php` du flux
  après redirection, refus systématique testé avec plusieurs variantes) ni
  le FFmpeg embarqué (build `--disable-network`) ne pouvaient le télécharger.
- **Solution** : nouveau moteur `HttpDownloadEngine` (reqwest, streaming,
  `Referer`/`Origin`, progression, écriture `.part`→rename) + `CompositeEngine`
  qui route entre lui et `YtDlpEngine` selon `EngineRequest.direct_url`.
  `RegistryPlanner` bascule en direct HTTP quand : provider résolu = generic
  (aucun site connu) **et** format non-manifeste (mp4/webm/mov, pas
  HLS/DASH) — les manifestes continuent d'aller à yt-dlp normalement.
  `Download` gagne `referer` (migration `0003_download_referer.sql`) —
  transmis par le widget (`source.pageUrl`) au démarrage du DL.
  Extension : `content-script.js` scanne le HTML de la page (regex
  `get_file/.../NNNp.mp4`, sélectionne la meilleure qualité), envoie un
  message `vdl_direct_file` → traité comme une détection normale.
  86 tests backend (dont un test d'intégration téléchargement réel via
  serveur TCP local + 3 tests de routage planner), clippy clean.
  **Généralise au-delà de PimpBunny** : tout site avec un lien fichier direct
  que le générique yt-dlp refuse en bénéficie désormais automatiquement.
  **Fix suivi (2026-07-21, test réel utilisateur)** : la détection KVS
  fonctionnait (`[VDL] direct file found on page`) mais rien n'apparaissait
  dans le widget — le message passait par `kind:"detection"` qui **sonde**
  l'URL, et le probe échouait (le CDN KVS rejette une requête serveur nue sans
  les entêtes/cookies exacts du navigateur). Fix : nouveau `kind:"direct"` →
  `IngestDetection::ingest_direct` qui crée la source **sans probe** (on sait
  déjà que c'est une vidéo, extraite du HTML), format progressif → routée vers
  le téléchargeur HTTP direct avec le Referer de la page. Même principe que
  `kind:"page"` (YouTube). 91 tests backend.

**Trois améliorations (2026-07-21, sur demande utilisateur)** :
1. **Visibilité des erreurs de DL** : en cas d'échec, la raison concise (ligne
   `ERROR:` de yt-dlp distillée par `concise_error()`, ex. « Video
   unavailable ») remonte via `DomainEvent::TaskStateChanged { error }` →
   `TaskStateChangedPayload.error` → `downloadsStore` → affichée en rouge dans
   le widget (avec bouton « Retirer ») et la page Téléchargements. Fini
   l'échec silencieux.
2. **Qualité par téléchargement** : sélecteur (Auto/1080p/720p/480p/Audio) par
   entrée dans le widget. `Download` gagne `max_height`/`audio_only`
   (migration `0002_download_quality.sql`, ALTER TABLE non destructif) ; le
   `RegistryPlanner` applique la qualité par-download en surcharge du défaut
   global (`download.max_height.or(global)`, `audio_only ||`). La page
   Détection standard reste sur le défaut global.
3. **Mise à jour yt-dlp** : commande Tauri `update_ytdlp` (bouton dans
   Réglages → Maintenance) qui lance `python -m pip install --upgrade yt-dlp`
   (le yt-dlp embarqué est un lanceur pip sur le Python système) puis renvoie
   la version obtenue. Erreur claire si Python absent.
   78 tests backend, 19 tests frontend, clippy clean.

**YouTube : fusion audio/vidéo (FFmpeg) + runtime JS (2026-07-19)** — après que
la détection de page marche, le téléchargement YouTube produisait **4 fichiers
séparés** au lieu d'un. Deux causes, deux fixes :
1. **FFmpeg absent** → yt-dlp ne pouvait pas fusionner les flux DASH
   audio/vidéo séparés de YouTube (`.f399.mp4` + `.f251.webm`). FFmpeg statique
   autonome (build BlueStacks n4.4.4, 14 Mo, un seul exe, mp4+matroska) copié
   dans `apps/src-tauri/binaries/ffmpeg.exe` ; l'engine le passe via
   `--ffmpeg-location` (câblage `ffmpeg_dir` dans `desktop/mod.rs`, conditionnel
   à la présence du fichier). **⚠️ Piège** : le FFmpeg de Shutter Encoder est un
   build à DLL partagées (400 Ko + 16 DLL) — l'exe seul ne tourne pas ; il faut
   un build STATIQUE. Vérifié en copiant l'exe seul dans un dossier isolé.
2. **Runtime JS requis** → yt-dlp 2025+ exige un runtime JS (deno par défaut)
   pour résoudre le challenge de signature YouTube ; sans lui, tous les clients
   de lecture échouent (« Video unavailable »). Node.js (déjà installé) branché
   via `--js-runtimes node:<chemin>` (`resolve_node_runtime()` résout
   `%ProgramFiles%\nodejs\node.exe` puis scanne le PATH ; `None` si absent →
   YouTube ne marche pas mais le reste oui).
   **Prouvé de bout en bout** : téléchargement complet de « Me at the zoo »
   (jNQXAC9IVRw) → yt-dlp télécharge f395+f251 puis **fusionne en un seul
   fichier** via le FFmpeg embarqué (`[Merger] Merging formats`).
   **Note** : la vidéo サカナクション testée par l'utilisateur (a8dgNdJV1uc) est
   géo/label-restreinte (« Video unavailable » sur tous les clients, VPN actif)
   — restriction côté YouTube, pas un bug de l'app. 73 tests backend.

**YouTube via détection de page native (2026-07-19)** : sniffer les URLs
`googlevideo` est la mauvaise approche pour YouTube (flux audio/vidéo séparés,
signés, éphémères, inexploitables tels quels). À la place, l'extension
reconnaît qu'une page est **yt-dlp-native** (regex `NATIVE_PAGE_RE` :
youtube.com/watch, /shorts/, /live/, youtu.be) et envoie un message
`{kind:"page", pageUrl, tabId, title}`. Le backend
(`IngestDetection::ingest_page`) crée un `MediaSource` depuis l'URL de la page
**sans la sonder** (une page HTML échoue au probe), avec `source_tab_id`/
`page_url` pour le scoping widget. Au téléchargement, le `RegistryPlanner`
re-résout le provider par host → `YouTubeProvider` → yt-dlp télécharge et muxe
nativement (le `providerId` envoyé par le frontend est ignoré, seul le host
compte). Aucun changement de DTO/frontend. Nettoyage au passage : retrait de la
machinerie de sniffing googlevideo/worker/page-hook (world:MAIN non supporté
de façon fiable par Opera GX ; injection de script abandonnée) — l'extension
revient à 3 fichiers (background/content-script/options). 71 tests backend
(dont `page_message_yields_a_source_without_probing`), clippy clean.
**Limite connue** : dépend d'un yt-dlp à jour (YouTube change souvent son
extraction) ; la liste `NATIVE_PAGE_RE` est extensible pour d'autres sites
yt-dlp-natifs.

**Console noire ouverte pendant les téléchargements — corrigée (2026-07-19)** :
`yt-dlp` était spawné sans `CREATE_NO_WINDOW`, faisant flasher une fenêtre de
console à chaque lancement. Fix : `creation_flags(0x08000000)` sur la
`Command` Windows dans `ytdlp_engine.rs`.

**Écran noir au double-clic sur `video-dl.exe` — corrigé (2026-07-19)** :
l'utilisateur lançait l'exe compilé en mode **dev**, qui charge son UI depuis
`http://localhost:1420` (sans le serveur Vite du script de dev = fenêtre
noire). Fix définitif : build de **production**
(`.\scripts\dev-msvc.ps1 tauri build`, frontend embarqué, aucune dépendance
réseau), installé via l'installeur NSIS généré
(`bundle\nsis\Video DL_0.1.0_x64-setup.exe /S`) → raccourci propre dans le
menu Démarrer, `%LOCALAPPDATA%\Video DL\video-dl.exe`. Bloqué une fois par
`"icon": []` vide dans `tauri.conf.json` (le fichier icône existait déjà,
juste pas référencé) → corrigé.

**Limite de vérification connue :** l'outil de contrôle du bureau utilisé par
l'assistant ne reconnaît que les applications installées (menu Démarrer) et pas cet
exécutable de dev ad-hoc — impossible de cliquer/capturer la fenêtre native
directement en session autonome. **Reste à faire par l'utilisateur** : ouvrir la
fenêtre déjà lancée et vérifier à l'œil probe/pipeline complet/pause-annulation/
réglages dans la vraie UI (le code est câblé et le process est stable, mais ce
dernier maillon visuel n'a pas été cliqué par l'assistant).

**Playlists (2026-10-02)** — dernière brique de la Phase 8. Une URL de playlist
(YouTube `playlist?list=`, `watch?v=…&list=…`, chaîne `/@…`, `/channel/`…, Vimeo
showcase/album/channel) est **dépliée** puis chaque vidéo est mise en file comme
un téléchargement ordinaire — le pipeline engine/scheduler/progression est
réutilisé tel quel, et `--no-playlist` reste forcé côté engine.
- Port `PlaylistResolver` + `YtDlpPlaylistResolver`
  (`infrastructure/download/ytdlp_playlist.rs`) : `yt-dlp --flat-playlist -J`
  (une requête de métadonnées, rien de téléchargé, timeout 90 s). Pré-filtre
  `looks_like_playlist` : une URL non-playlist ne lance **aucun** process.
  Parseur pur testé : onglets de chaîne imbriqués aplatis, vidéos
  supprimées/privées ignorées, doublons retirés. Format vérifié contre le vrai
  yt-dlp sur une vraie playlist (10 entrées, titres/durées OK).
- Use cases `ResolvePlaylist` / `StartPlaylistDownload` ; méthode repo
  `create_playlist` **transactionnelle** (playlist + downloads + tasks +
  `playlist_items` — tables déjà présentes dans `0001_initial.sql`, pas de
  nouvelle migration).
- IPC : `resolve_playlist` (→ `PlaylistDto | null`) et
  `start_playlist_download` (+ ACL dans `build.rs` / `capabilities/default.json`).
- UI (page Détection) : l'URL est d'abord résolue comme playlist ; si c'en est
  une → carte `PlaylistCard` (cases à cocher, tout cocher/décocher, qualité
  Meilleure/1080/720/480/Audio, « Télécharger N vidéos »), plus
  « Cette vidéo seulement » pour un lien `watch?v=…&list=…`. Échec de
  résolution → retombe sur le probe normal.
- Tests : 98 backend (+8 unitaires, +1 SQLite), 41 frontend (+3).
- **À vérifier par l'utilisateur** dans la vraie fenêtre : coller une URL de
  playlist dans Détection.
- Limite : une « Mix » YouTube (`list=RD…`) est aussi proposée comme playlist
  (~25-50 entrées) — d'où le bouton « Cette vidéo seulement ».

## Avancement par phase (roadmap ADR)

| Phase | Sujet | État |
|---|---|---|
| 0 | Architecture (ADR) | ✅ Validé |
| 1 | Core Rust (domain/application/infra) | ✅ Fait, testé |
| 2 | IPC (contrat + types TS générés) | ✅ Fait, testé |
| 3 | SQLite (SQLx, migrations, reprise crash) | ✅ Fait, testé |
| 4 | yt-dlp (engine, parser progression) + scheduler | ✅ Fait, testé |
| 5 | Frontend (React/TS/Tailwind/Query/Zustand) | ✅ Fait, build OK |
| 6 | Détection (classify, HLS, DASH, probe HTTP) | ✅ Fait, testé |
| 7 | Providers (generic/youtube/reddit/vimeo + registry) | ✅ Fait, testé |
| — | **Assemblage Tauri (shell desktop)** | ✅ Fait — compile et tourne sous MSVC (voir ci-dessous) |
| 8 | Améliorations : options débit/sous-titres, persistance settings, téléchargements actifs, annulation/pause réelles, output_path + historique réel, **playlists** | ✅ Fait, testé (playlists non cliquées dans la vraie fenêtre) |
| 9 | Mode fenêtre standard/volatile (widget always-on-top) | ✅ Fait, testé |
| 10 | Détection live (extension Opera GX + pont WebSocket local) | ✅ Fait, testé (extension non vérifiée en conditions réelles — voir ci-dessous) |
| — | Proxy MITM (hudsucker + rcgen) | ❌ Abandonné (décision 2026-07-19, voir ADR 4.5) — remplacé par la Phase 10 |

**Phase 8 déjà fait** (testé) : options `DownloadOptions` (débit via `--limit-rate`,
sous-titres), persistance des réglages (`Settings` + `SettingsRepository` in-memory
& SQLite, use cases + commandes IPC `get_settings`/`update_settings`, page Réglages
branchée), `list_active` (vue Download+Task) exposée en IPC + hydratation UI, et
**annulation/pause qui interrompent réellement le process yt-dlp** (registre
`RunningTasks` + port `TaskController` + `CancellationToken` ; `kill_on_drop`).
**Phase 8 terminée** (playlists ajoutées le 2026-10-02, voir section dédiée ci-dessous).

Ajouté aussi (testé) : à la complétion, **`output_path` persisté sur le download**
(capturé du vrai yt-dlp via `--print-to-file after_move:filepath`) et **ligne
archivée dans la table `history`** (avec taille en octets) ; la page **Historique**
lit désormais la vraie table (`recent_history`) et affiche titre/chemin/taille/date.

**Vérifié bout-en-bout (réseau réel + vrai yt-dlp), tests `#[ignore]` dans
`tests/e2e_pipeline.rs`** :
- `real_download_pipeline_mp4` : probe → SQLite → yt-dlp → progression 0→100% →
  fichier 2,8 Mo sur disque → **output_path capturé** → **historique 1 entrée, 2 848 208 o**.
- `real_cancel_kills_ytdlp_mid_download` : annulation en cours → `run_task` revient
  en ~1,5 s (au lieu de ~95 s à débit bridé), état `Cancelled` → le process est bien tué.
Lancer : `cargo test --test e2e_pipeline -- --ignored --nocapture` (yt-dlp requis dans le PATH).

## Ce qui tourne aujourd'hui

- **Backend** : `.\scripts\dev.ps1 test` → 66 tests verts. Voir [BUILD.md](BUILD.md).
  - Domaine pur (entités, machine à états, value objects) + use cases + ports.
  - Persistance SQLite réelle avec migrations et reprise après crash.
  - Engine yt-dlp (spawn process, parse `--progress-template`, pause via kill+`--continue`).
  - Scheduler concurrent (sémaphore Tokio), file d'attente.
  - Détection : classification MIME/extension, parsing HLS (m3u8-rs) et DASH
    (quick-xml) avec détection de chiffrement/DRM, probe HTTP réel (reqwest).
  - Providers avec registry résolu au runtime ; le cœur ne dépend d'aucun provider.
  - Types TypeScript générés (`ts-rs`) dans `apps/frontend/src/types/`.
- **Frontend** : `npm --prefix apps/frontend run build` → OK. `run dev` → rendu
  vérifié (header, navigation, page Détection). Structure feature-first, client
  IPC typé, TanStack Query pour l'état serveur, Zustand pour l'état UI.

## Ce qu'il reste (par ordre de priorité)

Toutes les phases de la roadmap sont codées. Restent des vérifications réelles :
1. **Test manuel dans la vraie fenêtre** : probe, pipeline complet,
   pause/annulation, réglages, **playlists**.
2. **Extension Opera GX en conditions réelles** (Phase 10, jamais vérifiée dans
   le navigateur).
3. Rebuild + réinstallation de la version de production
   (`.\scripts\dev-msvc.ps1 tauri build`) pour embarquer les playlists.

## Notes d'environnement (important)

- Toolchain **GNU** utilisé (MSVC indisponible sans UAC) ; le nom d'utilisateur
  contient un `é` qui casse le linker MinGW → toolchain et sortie de build
  déplacés sur des chemins **ASCII** (`C:\Users\Public`). Tout est automatisé par
  `scripts/dev.ps1`. Détails et alternative MSVC : [BUILD.md](BUILD.md).
- Les types TS sont générés par `cargo test` (donc `.\scripts\dev.ps1 test` avant
  un build frontend depuis un checkout neuf).
