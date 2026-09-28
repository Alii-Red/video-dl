// Video DL — content script (MV3).
//
// The background service worker sees network requests (webRequest) but has
// no DOM access. This script reads the page's best-effort poster/duration —
// <video poster> or og:image, and <video>.duration once metadata has loaded —
// and forwards it to the background worker, which attaches it to whatever
// media URL it reports for this tab. Never fetches or generates anything;
// only reads what the page already has.

// A better title than the raw tab title, for sites where the tab title is the
// same for every video. anime-sama gives every episode of a season the one page
// title ("Dragon Ball Z - Saison 1 | Anime-Sama…"), so a bare title makes every
// episode collide on one filename — read the selected episode from the page's
// own <select id="selectEpisodes"> and fold it in ("Dragon Ball Z - Saison 1 -
// Episode 265"). Returns null on other sites (keep the normal tab-title path).
function titleOverride() {
  if (!/anime-sama/i.test(location.hostname)) return null;
  const base = (document.title || "").split("|")[0].trim();
  const sel = document.querySelector("#selectEpisodes");
  const ep =
    sel && sel.options[sel.selectedIndex]
      ? sel.options[sel.selectedIndex].text.trim()
      : null;
  if (base && ep) return `${base} - ${ep}`;
  return base || null;
}

function currentMeta() {
  let thumbnail = null;
  let durationSecs = null;
  const video = document.querySelector("video");
  if (video) {
    if (video.poster) thumbnail = video.poster;
    if (Number.isFinite(video.duration) && video.duration > 0) {
      durationSecs = Math.round(video.duration);
    }
  }
  if (!thumbnail) {
    const og = document.querySelector('meta[property="og:image"]');
    if (og && og.content) thumbnail = og.content;
  }
  return { thumbnail, durationSecs, title: titleOverride() };
}

function sendMeta() {
  const meta = currentMeta();
  if (meta.thumbnail || meta.durationSecs || meta.title) {
    chrome.runtime.sendMessage({ type: "vdl_meta", ...meta }).catch(() => {});
  }
}

// The in-page "scope" that distinguishes otherwise same-URL videos — the
// selected anime-sama episode. The background worker folds this into the page
// URL for the context signal and each detection, so switching episode re-scopes
// the widget (and clears the previous episode's cards) even though the URL never
// changes. Null on sites without such a control.
function currentScope() {
  if (!/anime-sama/i.test(location.hostname)) return null;
  const sel = document.querySelector("#selectEpisodes");
  return sel && sel.options[sel.selectedIndex]
    ? sel.options[sel.selectedIndex].text.trim()
    : null;
}

function sendScope() {
  chrome.runtime
    .sendMessage({ type: "vdl_scope", scope: currentScope() || "" })
    .catch(() => {});
}

sendScope();
document.addEventListener("DOMContentLoaded", sendScope);

// The episode <select> changes the video without a full reload (same URL), so
// re-send the scope (to re-scope the widget) and the title as soon as the user
// picks another episode — before its new stream is sniffed.
document.addEventListener(
  "change",
  (e) => {
    if (e.target && e.target.id === "selectEpisodes") {
      sendScope();
      setTimeout(sendMeta, 300);
    }
  },
  true,
);

sendMeta();
document.addEventListener("loadedmetadata", sendMeta, true);
document.addEventListener("DOMContentLoaded", sendMeta);

// Re-check periodically for the first while — many players attach the
// <video> element or set its poster only after their JS bootstraps, well
// after document_idle.
let attempts = 0;
const interval = setInterval(() => {
  sendMeta();
  if (++attempts >= 20) clearInterval(interval);
}, 1000);

// --- KVS/tube-player direct links --------------------------------------
// Sites built on the common "KVS" tube player (kt_player.js — PimpBunny and
// many similar sites) embed the direct file URL(s) right in the page's HTML/
// inline scripts (a `/get_file/.../NNNp.mp4/` link per quality), rather than
// firing a network request our webRequest listener could see. webRequest
// also can't see these players' HLS *fallback* traffic reliably (it often
// runs inside a Worker) — but the direct link needs no network sniffing at
// all: it's sitting in the page source, redirects straight to the real file
// (verified: 302 → 200 `video/mp4`), and just needs the page's own URL as
// the Referer to work. We pick the single highest-quality link per page.
const KVS_LINK_RE = /https?:\/\/[^"'\s\\<>]+\/get_file\/[^"'\s\\<>]*?\.(mp4|webm|mov)\/?/gi;

function qualityOf(url) {
  const m = url.match(/_(\d{3,4})p\b/);
  return m ? parseInt(m[1], 10) : 0; // no explicit quality tag = lowest priority
}

// The video's real title, not the tab title. On KVS/tube sites document.title
// is often the playlist/model name ("Zoey U") with a " | Site" suffix, while
// og:title is the actual clip title. Prefer og:title; fall back to a cleaned
// document.title.
function videoTitle() {
  const og = document.querySelector('meta[property="og:title"]');
  if (og && og.content && og.content.trim()) return og.content.trim();
  const t = (document.title || "").replace(/\s*[|\-–—]\s*[^|\-–—]*$/, "").trim();
  return t || document.title || null;
}

let lastSentUrl = null; // dedupe: one direct link per (page URL) load

function scanKvsLinks() {
  const html = document.documentElement.outerHTML;
  const matches = html.match(KVS_LINK_RE);
  if (!matches || matches.length === 0) return;
  const unique = [...new Set(matches)];
  unique.sort((a, b) => qualityOf(b) - qualityOf(a));
  const best = unique[0];
  if (best === lastSentUrl) return;
  lastSentUrl = best;
  chrome.runtime
    .sendMessage({
      type: "vdl_direct_file",
      url: best,
      referer: location.href,
      title: videoTitle(),
    })
    .catch(() => {});
}

// Scan repeatedly for a good while — KVS players inject the player (and its
// get_file links) a few seconds after load, sometimes only once the poster is
// clicked. Deduped by URL, so extra scans are cheap.
let kvsAttempts = 0;
const kvsInterval = setInterval(() => {
  scanKvsLinks();
  if (++kvsAttempts >= 30) clearInterval(kvsInterval);
}, 1000);
scanKvsLinks();
document.addEventListener("DOMContentLoaded", scanKvsLinks);

// SPA navigation (clicking another video without a full reload) keeps this
// same content script alive — re-scan whenever the URL changes so the new
// video is picked up without the user having to reload the page. Also poll the
// in-page scope (anime-sama episode): the "Épisode suivant/précédent" buttons
// and the dropdown change it *programmatically*, which fires no `change` event,
// so a plain listener misses it — polling catches every kind of switch. The
// background delays each detection ~1.2s, so updating the scope here (well
// within that window) means the new episode's stream is tagged correctly.
let currentUrl = location.href;
let lastScope = currentScope();
setInterval(() => {
  if (location.href !== currentUrl) {
    currentUrl = location.href;
    lastSentUrl = null; // new page → allow a fresh detection
    kvsAttempts = 0;
    // Give the new page's player a moment to inject its links, then scan.
    let n = 0;
    const t = setInterval(() => {
      scanKvsLinks();
      sendMeta();
      if (++n >= 15) clearInterval(t);
    }, 700);
  }
  const scope = currentScope();
  if (scope !== lastScope) {
    lastScope = scope;
    sendScope();
    sendMeta(); // refresh the episode-tagged title too
  }
}, 400);

// Explicit re-scan requested from the app's widget refresh button (relayed by
// the background worker over the WS channel).
chrome.runtime.onMessage.addListener((msg) => {
  if (msg && msg.type === "vdl_rescan") {
    lastSentUrl = null;
    lastScope = currentScope();
    sendScope();
    scanKvsLinks();
    sendMeta();
  }
});
