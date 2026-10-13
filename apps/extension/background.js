// Video DL — live detection service worker (MV3).
//
// Two detection paths, both pushed to the local Video DL app over a WebSocket:
//
//  1. Direct media sniffing (webRequest): manifests/files whose URL ends in a
//     known media extension (.m3u8/.mpd/.mp4/...). Works for sites that fetch
//     their stream on the main thread (anime-sama, etc.).
//
//  2. yt-dlp-native page detection: for sites yt-dlp handles natively
//     (YouTube), sniffing the raw stream is the wrong tool — the media URLs
//     are split audio/video, signed and short-lived. Instead we detect that
//     the active tab IS such a page and offer the PAGE URL directly; the app
//     hands it to yt-dlp, which downloads and muxes it properly.
//
// The MV3 service worker is ephemeral (the browser may kill it when idle), so
// the WebSocket is (re)opened lazily and reconnected with backoff whenever it
// drops. Config (port + token) lives in chrome.storage.local, set via the
// options page and copied from the app's Réglages screen.

function log(...args) {
  console.log("[VDL]", ...args);
}

const MEDIA_RE = /\.(m3u8|mpd|mp4|m4v|webm|mov)(\?|#|$)/i;
// Ad creatives are real video files (.webm banners, preroll .mp4s...) that
// legitimately match MEDIA_RE — without this, the widget offers the site's
// ads for download. Path-based heuristic: these slugs ~never appear in real
// content URLs.
const AD_RE = /\/(bnr|banner|banners|ad|ads|advert|adverts|advertising|promo|preroll|prerolls|popunder|sponsor|creatives?)\//i;

// Pages yt-dlp handles natively — detected by URL so we can offer the page
// itself for download instead of sniffing its raw streams (which are either
// unusable, like YouTube's split audio/video, or arrive as hundreds of HLS
// segments, like Twitter/X — one .mp4 fragment per chunk that would each
// register as a separate detection). Extend this list for other yt-dlp-native
// sites as needed.
const NATIVE_PAGE_RE =
  /^https?:\/\/(www\.|m\.|mobile\.)?(youtube\.com\/(watch\?|shorts\/|live\/)|youtu\.be\/|reddit\.com\/r\/[^/]+\/comments\/|vimeo\.com\/\d+)/i;

// Sites we deliberately never detect on. X/Twitter is a SPA that rewrites the
// URL to a post as you merely scroll or click through the feed, and its
// timeline autoplays clips — so both the native-page path and the .mp4-sniffing
// path would surface "random" videos the user never chose to open. Out of the
// app's provider scope anyway (generic/youtube/reddit/vimeo). Paste an X URL by
// hand if you ever actually want one. Matched against the page URL (to suppress
// sniffing while on X) and against the media URL host video.twimg.com (belt and
// braces, in case the tab context isn't set yet).
const BLOCKED_PAGE_RE = /^https?:\/\/(www\.|m\.|mobile\.)?(twitter|x)\.com\//i;
const BLOCKED_MEDIA_RE = /(^|\/\/|\.)twimg\.com\//i;

const DEFAULTS = { port: 8787, token: "" };

// Media URLs sometimes get re-signed on each request (rotating token/expiry) —
// deduping on origin+pathname (ignoring the query) collapses those to one
// entry. Two distinct videos ~never share a literal path.
function dedupeKey(url) {
  try {
    const u = new URL(url);
    return `${u.origin}${u.pathname}`;
  } catch (e) {
    return url;
  }
}

let socket = null;
let connecting = false;
let backoff = 500;
const pending = []; // buffered detections while the socket is (re)connecting
const reported = new Set(); // per-worker-lifetime dedupe

// Best-effort poster/duration per tab, pushed by content-script.js (which has
// DOM access this service worker doesn't). Attached to whatever media URL we
// report for that tab; may be stale/absent, that's fine — it's cosmetic.
const tabMeta = new Map();

// Optional per-tab scope suffix (e.g. anime-sama's selected "Episode 266"),
// pushed by the content script. Folded into the page URL used for BOTH the
// context signal and each detection's `pageUrl`, so a site that keeps one URL
// across a whole series (anime-sama switches episode without a navigation) still
// scopes each episode distinctly — the app then drops the previous episode's
// cards when the scope changes instead of piling them up.
const tabSuffix = new Map();

function scopedUrl(tabId, rawUrl) {
  if (!rawUrl) return rawUrl || null;
  const sfx = tabSuffix.get(tabId);
  return sfx ? `${rawUrl}#vdl:${encodeURIComponent(sfx)}` : rawUrl;
}

chrome.runtime.onMessage.addListener((msg, sender) => {
  if (!msg || !sender.tab || !(sender.tab.id >= 0)) return;
  if (msg.type === "vdl_scope") {
    // Content script reporting the current in-page scope (episode). On a real
    // change, re-emit the context so the app re-scopes and clears stale cards.
    const tabId = sender.tab.id;
    const scope = msg.scope || null;
    if ((tabSuffix.get(tabId) || null) !== scope) {
      if (scope) tabSuffix.set(tabId, scope);
      else tabSuffix.delete(tabId);
      lastSentContext = null; // force re-send even if the raw URL is unchanged
      chrome.tabs
        .get(tabId)
        .then((t) => sendContext(tabId, t.url || null))
        .catch(() => {});
    }
    return;
  }
  if (msg.type === "vdl_meta") {
    const existing = tabMeta.get(sender.tab.id) || {};
    tabMeta.set(sender.tab.id, {
      thumbnail: msg.thumbnail || existing.thumbnail || null,
      durationSecs: msg.durationSecs || existing.durationSecs || null,
      // A page-derived title override (e.g. anime-sama's series + episode);
      // a fresh non-null value wins so switching episode updates it.
      title: msg.title || existing.title || null,
    });
  } else if (msg.type === "vdl_direct_file" && typeof msg.url === "string") {
    // Found in the page's own HTML (KVS/tube player) — not something
    // webRequest ever sees, so it bypasses the sniffing/dedupe path
    // entirely and reports straight away. `referer` is the page itself,
    // right here and now — more reliable than the async tab-context cache
    // for a page that may have just loaded this very tab.
    log("direct file found on page ->", msg.url);
    const meta = tabMeta.get(sender.tab.id) || {};
    // kind: "direct" — the app must NOT probe this (the CDN rejects a bare
    // server-side request); it downloads it straight over HTTP with the page
    // as Referer. See IngestDetection::ingest_direct.
    send({
      kind: "direct",
      mediaUrl: msg.url,
      tabId: sender.tab.id,
      pageUrl: msg.referer || null,
      title: msg.title || "",
      posterUrl: meta.thumbnail || null,
      durationSecs: meta.durationSecs || null,
    });
  }
});

chrome.tabs.onRemoved.addListener((tabId) => {
  tabMeta.delete(tabId);
  tabPageUrl.delete(tabId);
  tabSuffix.delete(tabId);
});

// --- Active tab/page tracking -------------------------------------------
// Lets the app scope the widget's display to whatever's on screen right now
// (tab switch or navigation), without discarding older detections — see
// ADR 4.6 follow-up. Re-derived on worker wake since MV3 workers are
// ephemeral and don't retain state across restarts.
//
// `tabPageUrl` is the single source of truth for "what page is this tab on"
// — both the context message AND every detection's `pageUrl` read from it,
// rather than each independently re-querying chrome.tabs.get() at a
// different moment.
let activeTabId = null;
const tabPageUrl = new Map();
let lastSentContext = null;

function sendContext(tabId, pageUrl) {
  if (tabId == null) return;
  tabPageUrl.set(tabId, pageUrl || null); // raw URL (native-page checks use it)
  maybeSendNativePage(tabId, pageUrl);
  const scoped = scopedUrl(tabId, pageUrl || null);
  const key = `${tabId}|${scoped || ""}`;
  if (key === lastSentContext) return; // avoid spamming on repeated events
  lastSentContext = key;
  // A genuine navigation/tab-switch/scope-change — start this page's media
  // dedupe fresh so the new episode's streams aren't swallowed as duplicates.
  reported.clear();
  scheduled.clear();
  log("context ->", tabId, scoped);
  send({ kind: "context", tabId, pageUrl: scoped });
}

// When the active page is one yt-dlp handles natively, offer the page URL
// itself as a downloadable item (no raw-stream sniffing). Sent on every
// trigger (navigation, full page load, SPA nav, manual refresh) — the app
// dedupes detections by URL, so re-sending the same page is harmless and lets
// a plain page reload (F5, same URL) re-surface it.
async function maybeSendNativePage(tabId, pageUrl) {
  if (!pageUrl || !NATIVE_PAGE_RE.test(pageUrl)) return;
  // The widget only shows a page detection when the app's *active context*
  // matches its URL exactly. Send a context with the same URL right before
  // the detection so they can never disagree (a stale context — e.g. after a
  // reload or a worker restart — would otherwise filter the detection out and
  // nothing would appear).
  tabPageUrl.set(tabId, pageUrl);
  lastSentContext = `${tabId}|${pageUrl}`;
  send({ kind: "context", tabId, pageUrl });
  let title = "";
  try {
    const tab = await chrome.tabs.get(tabId);
    title = tab.title || "";
  } catch (e) {}
  log("native page ->", pageUrl);
  send({ kind: "page", pageUrl, tabId, title });

  // On SPA navigation (YouTube changes video without a reload) document.title
  // lags the URL change by a moment, so the title captured just above is often
  // the *previous* page's ("Historique de visionnage - YouTube" when you open a
  // video from your history). Re-read it a beat later and re-emit if it changed
  // — the app merges the fresher title into the same detection (same URL).
  scheduleTitleRefresh(tabId, pageUrl, title);
}

// Guard against stacking timers when many triggers fire for the same page.
const titleRefreshTimers = new Map();

function scheduleTitleRefresh(tabId, pageUrl, seenTitle) {
  const key = `${tabId}|${pageUrl}`;
  clearTimeout(titleRefreshTimers.get(key));
  titleRefreshTimers.set(
    key,
    setTimeout(async () => {
      titleRefreshTimers.delete(key);
      try {
        const tab = await chrome.tabs.get(tabId);
        // Only re-emit if we're still on the same page and the title actually
        // caught up to something different.
        if (tab.url === pageUrl && tab.title && tab.title !== seenTitle) {
          log("native page title refreshed ->", tab.title);
          send({ kind: "page", pageUrl, tabId, title: tab.title });
        }
      } catch (e) {}
    }, 1500),
  );
}

async function refreshActiveTab() {
  try {
    const [tab] = await chrome.tabs.query({ active: true, lastFocusedWindow: true });
    if (tab) {
      activeTabId = tab.id;
      sendContext(activeTabId, tab.url || null);
    }
  } catch (e) {}
}

chrome.tabs.onActivated.addListener(async ({ tabId }) => {
  activeTabId = tabId;
  try {
    const tab = await chrome.tabs.get(tabId);
    sendContext(tabId, tab.url || null);
  } catch (e) {
    sendContext(tabId, null);
  }
});

// Classic navigation (full page load / URL change) in the active tab.
chrome.tabs.onUpdated.addListener((tabId, changeInfo) => {
  if (tabId !== activeTabId) return;
  if (changeInfo.url) {
    sendContext(tabId, changeInfo.url);
  } else if (changeInfo.status === "complete") {
    // A plain reload (F5) doesn't change the URL, so `changeInfo.url` never
    // fires — but a native-page detection still needs re-emitting (the widget
    // was cleared / the worker may have restarted). Re-run it on load
    // completion.
    maybeSendNativePage(tabId, tabPageUrl.get(tabId));
  }
});

// SPA-style navigation (history.pushState) in the active tab's top frame —
// YouTube changes video without a full reload.
if (chrome.webNavigation) {
  chrome.webNavigation.onHistoryStateUpdated.addListener((details) => {
    if (details.frameId !== 0 || details.tabId !== activeTabId) return;
    sendContext(details.tabId, details.url);
  });
}

refreshActiveTab();

async function config() {
  const stored = await chrome.storage.local.get(["port", "token"]);
  return { port: stored.port || DEFAULTS.port, token: stored.token || DEFAULTS.token };
}

// Zero-config: if the extension has never been configured (no token stored),
// discover the app's port+token from its local /config endpoint. Only the
// extension (host permissions) can read that response — web pages are blocked by
// CORS — so the token stays secret. Tries the default port and a couple of
// neighbours in case the user picked another. Safe to call often: returns early
// once configured. Never overrides a token the user set manually.
async function autoConfigure() {
  const stored = await chrome.storage.local.get(["port", "token"]);
  if (stored.token) return;
  const ports = [...new Set([stored.port || DEFAULTS.port, 8787, 8788, 8789])];
  for (const p of ports) {
    try {
      const res = await fetch(`http://127.0.0.1:${p}/config`, { cache: "no-store" });
      if (!res.ok) continue;
      const cfg = await res.json();
      if (cfg && cfg.token) {
        await chrome.storage.local.set({ port: cfg.port || p, token: cfg.token });
        log("auto-configured from /config on port", cfg.port || p);
        return;
      }
    } catch (e) {
      // app not reachable on this port — try the next
    }
  }
}

// Make sure we're configured (auto-discover if needed), then (re)connect.
function connect() {
  autoConfigure().finally(() => ensureSocket());
}

async function ensureSocket() {
  if (socket && socket.readyState === WebSocket.OPEN) return;
  if (connecting) return;
  connecting = true;
  const { port, token } = await config();
  const url = `ws://127.0.0.1:${port}/?token=${encodeURIComponent(token)}`;
  try {
    socket = new WebSocket(url);
  } catch (e) {
    connecting = false;
    scheduleReconnect();
    return;
  }
  socket.addEventListener("open", () => {
    log("WS connected to", url.replace(/token=[^&]+/, "token=***"));
    connecting = false;
    backoff = 500;
    while (pending.length) socket.send(pending.shift());
  });
  socket.addEventListener("close", () => {
    log("WS closed, reconnecting with backoff", backoff, "ms");
    socket = null;
    connecting = false;
    scheduleReconnect();
  });
  socket.addEventListener("error", () => {
    // 'close' fires next; reconnect handled there.
  });
  // App → extension: the widget's refresh button asks us to re-scan the
  // active tab. Relay it to that tab's content script.
  socket.addEventListener("message", (e) => {
    try {
      const cmd = JSON.parse(e.data);
      if (cmd && cmd.cmd === "rescan") {
        log("rescan requested by app");
        // Query the active tab fresh (the worker may have just restarted with
        // empty state). Re-detect a yt-dlp-native page (YouTube/X/...) AND ask
        // the content script to re-scan for direct/KVS links.
        chrome.tabs
          .query({ active: true, lastFocusedWindow: true })
          .then(([tab]) => {
            if (!tab) return;
            activeTabId = tab.id;
            tabPageUrl.set(tab.id, tab.url || null);
            maybeSendNativePage(tab.id, tab.url || null);
            chrome.tabs.sendMessage(tab.id, { type: "vdl_rescan" }).catch(() => {});
            // For a plain media page (anime-sama, ...) the stream is sniffed
            // only while it loads — once it's already playing there's nothing
            // live to re-scan, so a refit can't surface it. Reload the tab to
            // replay the request. Skip yt-dlp-native pages (YouTube): their page
            // URL is offered directly, so a disruptive reload isn't needed.
            const url = tab.url || "";
            if (/^https?:/i.test(url) && !NATIVE_PAGE_RE.test(url)) {
              chrome.tabs.reload(tab.id).catch(() => {});
            }
          })
          .catch(() => {});
      }
    } catch (err) {}
  });
}

function scheduleReconnect() {
  backoff = Math.min(backoff * 2, 15000);
  setTimeout(() => ensureSocket(), backoff);
}

function send(payload) {
  const text = JSON.stringify(payload);
  if (socket && socket.readyState === WebSocket.OPEN) {
    socket.send(text);
  } else {
    if (pending.length < 100) pending.push(text);
    ensureSocket();
  }
}

async function report(mediaUrl, tabId) {
  const key = dedupeKey(mediaUrl);
  if (!mediaUrl || reported.has(key)) return;
  reported.add(key);
  // Scope the detection to the same page+episode key the context uses, so the
  // widget shows it under (and only under) the current episode.
  const pageUrl = scopedUrl(tabId, tabPageUrl.get(tabId) || null);
  let tabTitle = "";
  try {
    if (tabId >= 0) {
      const tab = await chrome.tabs.get(tabId);
      tabTitle = tab.title || "";
    }
  } catch (e) {
    // tab may be gone; send what we have
  }
  const meta = (tabId >= 0 && tabMeta.get(tabId)) || {};
  // Prefer a page-derived title (anime-sama episode, ...) over the raw tab
  // title, which is identical across a whole season.
  const title = meta.title || tabTitle;
  log("reporting", mediaUrl, "for tab", tabId, pageUrl);
  send({
    kind: "detection",
    mediaUrl,
    tabUrl: pageUrl || "",
    tabTitle: title,
    posterUrl: meta.thumbnail || null,
    durationSecs: meta.durationSecs || null,
    tabId,
    pageUrl,
  });
}

// `video.duration` isn't known the instant the manifest request fires — it
// becomes available once the player has parsed enough of it. Delay briefly
// and read tabMeta at send time so the content script's periodic ping can
// catch up. Scheduling (not the report) is deduped so a burst of requests for
// the same URL only delays once.
const scheduled = new Set();

function scheduleReport(mediaUrl, tabId) {
  const key = dedupeKey(mediaUrl);
  if (!mediaUrl || reported.has(key) || scheduled.has(key)) return;
  scheduled.add(key);
  setTimeout(() => {
    scheduled.delete(key);
    report(mediaUrl, tabId);
  }, 1200);
}

chrome.webRequest.onBeforeRequest.addListener(
  (details) => {
    // tabId < 0 means the request isn't tied to any tab — nothing the widget
    // can scope to a page, skip it.
    if (!details.url || details.tabId < 0) return;
    if (!MEDIA_RE.test(details.url)) return;
    // Never surface X/Twitter media (see BLOCKED_*_RE): the media host itself,
    // or anything sniffed while the tab is on X.
    if (BLOCKED_MEDIA_RE.test(details.url)) return;
    if (AD_RE.test(details.url)) {
      log("skipped ad-looking request", details.url);
      return;
    }
    // On a yt-dlp-native page (YouTube, X, ...) the page itself is already
    // offered for download — its raw stream requests (e.g. X's HLS .mp4
    // segments, hundreds per video) are noise that would flood the widget.
    const activePage = tabPageUrl.get(details.tabId);
    if (activePage && (NATIVE_PAGE_RE.test(activePage) || BLOCKED_PAGE_RE.test(activePage))) {
      return;
    }
    log("matched request", details.url);
    scheduleReport(details.url, details.tabId);
  },
  { urls: ["<all_urls>"] },
);

// --- MV3 keep-alive ------------------------------------------------------
// The service worker is ephemeral: the browser suspends it when idle and only
// wakes it on a registered event. Once suspended, the WebSocket to the app is
// gone and nothing reconnects it until the user happens to navigate — so if
// the app (re)starts while the user sits on an already-loaded page, the widget
// stays empty and even its refresh button is dead (rescan is pushed app→ext
// over that same dead socket). A periodic alarm wakes the worker on a fixed
// cadence; every wake re-runs this script (re-registering listeners) and the
// handler below re-opens the socket if it dropped. This is the standard MV3
// pattern for keeping a long-lived connection alive.
const KEEPALIVE_ALARM = "vdl-keepalive";
if (chrome.alarms) {
  chrome.alarms.create(KEEPALIVE_ALARM, { periodInMinutes: 0.5 });
  chrome.alarms.onAlarm.addListener((alarm) => {
    if (alarm.name === KEEPALIVE_ALARM) {
      // Also retries auto-config, so the extension self-configures as soon as
      // the app is running even if it wasn't when the extension first loaded.
      connect();
      // A wake is also a good moment to re-assert the active tab's context /
      // native-page detection, in case the app restarted and lost its state.
      refreshActiveTab();
    }
  });
}
if (chrome.runtime.onStartup) {
  chrome.runtime.onStartup.addListener(() => {
    connect();
    refreshActiveTab();
  });
}

// Auto-configure (if needed) and warm the connection when the worker wakes.
connect();
