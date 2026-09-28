# Video DL — extension de détection live (Opera GX)

Extension Manifest V3 qui observe les requêtes réseau de l'onglet actif et
pousse les URLs de médias/manifestes (`.m3u8`, `.mpd`, `.mp4`, …) vers l'app
Video DL locale via WebSocket. Voir **ADR 4.6**.

## Installation (mode développeur, non distribuée)

1. Lance l'app Video DL. Va dans **Réglages** : note le **port** et le **token**
   de l'extension (générés au premier lancement).
2. Dans Opera GX : `opera://extensions` → active le **Mode développeur** →
   **Charger l'extension décompressée** → sélectionne ce dossier
   (`apps/extension/`).
3. Ouvre la page d'options de l'extension → colle le **port** et le **token** →
   Enregistrer.
4. Navigue : dès qu'une vidéo se charge, elle apparaît dans Video DL (surtout
   pratique en **mode widget**, la petite fenêtre always-on-top).

## Fonctionnement

- `background.js` (service worker) : `chrome.webRequest.onBeforeRequest` en
  observation, pré-filtre par regex média, envoie
  `{ mediaUrl, tabUrl, tabTitle, posterUrl, durationSecs }` au serveur WS
  local. Reconnexion automatique (le service worker MV3 est éphémère).
- `content-script.js` : lit `<video poster>` / `og:image` et
  `<video>.duration` sur la page (le service worker n'a pas accès au DOM),
  les transmet à `background.js` qui les attache au prochain flux détecté sur
  cet onglet. Best-effort — jamais bloquant, jamais de requête réseau
  supplémentaire.
- Côté app : `ExtensionWsBridge` (bind `127.0.0.1` + token) → `IngestDetection`
  → `probe` → événement `MediaDetected` → même pipeline que le reste. Le
  poster/durée atterrissent sur `MediaSource.thumbnail`/`duration_secs`,
  affichés dans le widget.

**Après une mise à jour de l'extension** (changement de `manifest.json`,
`background.js` ou `content-script.js`) : recharge-la dans
`opera://extensions` (bouton ↻) — Opera GX ne recharge pas automatiquement une
extension chargée en mode développeur.

## Sécurité

- Connexion en `127.0.0.1` uniquement + token dans le handshake.
- L'extension ne peut que **proposer** des URLs au probe ; un téléchargement
  reste toujours un clic dans l'app. Rien ne quitte la machine.
- Aucune donnée envoyée ailleurs, aucune télémétrie, extension jamais publiée
  sur un store.
