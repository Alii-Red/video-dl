(function () {
  "use strict";

  // Visible in the WebView2 DevTools console (right-click the extraction
  // window > Inspect, or F12) — lets us see exactly what the hook observed
  // without needing to guess. Every message is prefixed "[VDL]" so it's easy
  // to filter.
  function log() {
    try {
      var args = Array.prototype.slice.call(arguments);
      console.log.apply(console, ["[VDL]"].concat(args));
    } catch (e) {}
  }

  // Candidate media/manifest URLs, by extension or query-less suffix. Kept
  // deliberately permissive at the extension level (probe_candidates on the
  // Rust side re-validates via HTTP HEAD/GET + parsing, so a false positive
  // here just costs one wasted probe, not a wrong download).
  var MEDIA_RE = /\.(m3u8|mpd|mp4|m4v|webm|mov)(\?|#|$)/i;
  var reported = new Set();

  // A nested (cross-origin) frame can't reach Tauri directly, so it hands the
  // URL to the top frame via postMessage; only the top frame ever calls
  // invoke(). Separately — even on the top frame — `window.__TAURI__` may not
  // exist YET at the exact moment this script runs: our initialization
  // script and Tauri's own core bootstrap are both registered to run "on
  // document created", with no guaranteed order between them. So this isn't
  // a one-shot check: retry briefly before giving up, and — critically —
  // actually handle invoke()'s returned promise. It was previously fired
  // without a .catch(), so a permission/capability rejection from Tauri's ACL
  // would have failed completely silently.
  var isTopFrame = window === window.top;

  // Best-effort poster/duration for the widget's thumbnail — read from
  // whatever <video> element the page already has, or the og:image meta as
  // a fallback poster. Never fetched or generated; just whatever's on the
  // DOM at report time.
  function pageMeta() {
    var thumbnail = null;
    var durationSecs = null;
    try {
      var video = document.querySelector("video");
      if (video) {
        if (video.poster) thumbnail = video.poster;
        if (isFinite(video.duration) && video.duration > 0) {
          durationSecs = Math.round(video.duration);
        }
      }
      if (!thumbnail) {
        var og = document.querySelector('meta[property="og:image"]');
        if (og && og.content) thumbnail = og.content;
      }
    } catch (e) {}
    return { thumbnail: thumbnail, durationSecs: durationSecs };
  }

  function invokeReport(url, attempt) {
    attempt = attempt || 0;
    if (window.__TAURI__ && window.__TAURI__.core) {
      // invokeReport only ever runs in the top frame (see report() below), so
      // document.title here is genuinely the extracted page's title — used as
      // a human-readable download filename instead of the raw manifest URL.
      var meta = pageMeta();
      window.__TAURI__.core
        .invoke("report_detected_url", {
          url: url,
          title: document.title || null,
          thumbnail: meta.thumbnail,
          durationSecs: meta.durationSecs,
        })
        .then(function () {
          log("invoke() succeeded for", url);
        })
        .catch(function (err) {
          log("invoke() REJECTED for", url, "—", err);
        });
      return;
    }
    if (attempt < 20) {
      setTimeout(function () {
        invokeReport(url, attempt + 1);
      }, 100);
      return;
    }
    log("Tauri IPC bridge never became available on the TOP frame — cannot report", url);
  }

  function report(url) {
    if (!url || typeof url !== "string") return;
    if (!MEDIA_RE.test(url)) return;
    if (reported.has(url)) return;
    reported.add(url);
    log("reported:", url);
    try {
      if (isTopFrame) {
        invokeReport(url);
      } else {
        log("nested frame — forwarding to top frame:", url);
        window.top.postMessage({ __vdl_frame_report: url }, "*");
      }
    } catch (e) {
      log("report() failed:", e);
    }
  }

  // Only the top frame acts on forwarded reports — a nested frame receiving
  // one would just hit the same missing-bridge problem again.
  if (isTopFrame) {
    window.addEventListener("message", function (e) {
      if (e && e.data && typeof e.data.__vdl_frame_report === "string") {
        report(e.data.__vdl_frame_report);
      }
    });
  }

  log("hook installed on", location.href, isTopFrame ? "(top frame)" : "(nested frame)");

  // --- fetch -----------------------------------------------------------
  var originalFetch = window.fetch;
  if (originalFetch) {
    window.fetch = function (input, init) {
      try {
        var url = typeof input === "string" ? input : input && input.url;
        report(url);
      } catch (e) {}
      return originalFetch.apply(this, arguments);
    };
  }

  // --- XMLHttpRequest ----------------------------------------------------
  var originalOpen = XMLHttpRequest.prototype.open;
  XMLHttpRequest.prototype.open = function (method, url) {
    try {
      report(url);
    } catch (e) {}
    return originalOpen.apply(this, arguments);
  };

  // --- <video>/<source> elements, including ones inserted after load -----
  function scanMediaElements(root) {
    try {
      var els = root.querySelectorAll("video[src], source[src], video source[src]");
      for (var i = 0; i < els.length; i++) {
        report(els[i].src);
      }
    } catch (e) {}
  }

  document.addEventListener("DOMContentLoaded", function () {
    scanMediaElements(document);
  });
  // Page may already be loaded by the time the script runs.
  scanMediaElements(document);

  try {
    var observer = new MutationObserver(function () {
      scanMediaElements(document);
    });
    observer.observe(document.documentElement || document, {
      childList: true,
      subtree: true,
      attributes: true,
      attributeFilter: ["src"],
    });
  } catch (e) {}

  // --- Web Workers ---------------------------------------------------
  // Many HLS/DASH players (hls.js, dash.js, custom loaders) fetch segments
  // and manifests from inside a dedicated Worker for performance, then hand
  // the assembled result to the <video> element as a `blob:` URL. A Worker
  // has its own global scope with its own `fetch`/`XMLHttpRequest`, so the
  // page-level patches above never see those requests.
  //
  // We can't inject a script into an *existing* worker, but we CAN intercept
  // `new Worker(url)` calls: wrap the target script in a small shim (loaded
  // via a blob: URL) that patches self.fetch/XHR exactly like above, then
  // `importScripts()`s the real worker code. Classic workers only —
  // `{type: 'module'}` workers can't use importScripts, so those fall back
  // to an unwrapped worker rather than risk breaking playback.
  try {
    var OriginalWorker = window.Worker;
    if (OriginalWorker) {
      window.Worker = function (scriptURL, options) {
        if (options && options.type === "module") {
          log("module worker (unwrapped, importScripts unsupported):", scriptURL);
          return new OriginalWorker(scriptURL, options);
        }
        try {
          var abs = new URL(scriptURL, location.href).href;
          log("wrapping worker:", abs);
          var shim =
            "self.fetch = (function(orig){return function(input){" +
            "try{var u=typeof input==='string'?input:(input&&input.url);if(u)self.postMessage({__vdl_url:u});}catch(e){}" +
            "return orig.apply(this,arguments);};})(self.fetch);" +
            "var __vdlOpen = XMLHttpRequest.prototype.open;" +
            "XMLHttpRequest.prototype.open=function(m,u){" +
            "try{if(u)self.postMessage({__vdl_url:u});}catch(e){}" +
            "return __vdlOpen.apply(this,arguments);};" +
            "try{importScripts(" + JSON.stringify(abs) + ");}catch(e){self.postMessage({__vdl_wrap_failed:String(e)});}";
          var blobUrl = URL.createObjectURL(
            new Blob([shim], { type: "application/javascript" }),
          );
          var worker = new OriginalWorker(blobUrl, options);
          worker.addEventListener("message", function (e) {
            if (e && e.data && e.data.__vdl_url) {
              report(e.data.__vdl_url);
            } else if (e && e.data && e.data.__vdl_wrap_failed) {
              log("worker wrap failed inside importScripts:", e.data.__vdl_wrap_failed);
            }
          });
          worker.addEventListener("error", function (e) {
            log("wrapped worker errored:", e && e.message);
          });
          return worker;
        } catch (e) {
          // Wrapping failed (e.g. cross-origin script importScripts blocked
          // by CORS) — fall back to a real, unwrapped worker so the page's
          // own playback still works; we just won't see its requests.
          log("worker wrap threw synchronously, falling back unwrapped:", e);
          return new OriginalWorker(scriptURL, options);
        }
      };
      window.Worker.prototype = OriginalWorker.prototype;
    }
  } catch (e) {}
})();
