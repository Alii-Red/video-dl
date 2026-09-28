import { create } from "zustand";
import type { MediaSourceDto } from "../types/MediaSourceDto";

interface DetectionsState {
  /** Detected-but-not-yet-downloaded sources, newest first, deduped by URL.
   * Never trimmed by browsing-context changes — that's a display-only
   * concern (see `activeTabId`/`activePageUrl` below), so nothing is lost if
   * the user navigates away by accident. */
  items: MediaSourceDto[];
  /** The Opera GX extension's currently active tab/page, if known — used to
   * scope which `items` the volatile widget actually shows. `null` until the
   * extension has reported a context at least once. */
  activeTabId: number | null;
  activePageUrl: string | null;
  add: (source: MediaSourceDto) => void;
  remove: (url: string) => void;
  clear: () => void;
  setActiveContext: (tabId: number, pageUrl: string | null) => void;
}

/**
 * Global store of live media detections, fed by the `media://detected` event
 * (published by the backend probe pipeline — manual probe, page extraction,
 * and, in Phase 10, the Opera GX extension). Lets the compact volatile widget
 * and the detection page both react to detections without a second round-trip.
 */
export const useDetectionsStore = create<DetectionsState>((set) => ({
  items: [],
  activeTabId: null,
  activePageUrl: null,
  add: (source) =>
    set((s) => {
      const idx = s.items.findIndex((i) => i.url === source.url);
      if (idx === -1) return { items: [source, ...s.items] };
      // Same URL already known — a later observation of the same source
      // (e.g. a widget refresh re-emitting a YouTube page detection once
      // document.title has caught up) carries fresher cosmetic metadata.
      // Merge it in-place (keeping list position), letting non-null incoming
      // fields win so a better title/thumbnail/duration replaces a stale or
      // missing one, without wiping good data when the re-emit omits a field.
      const prev = s.items[idx];
      const merged: MediaSourceDto = {
        ...prev,
        title: source.title ?? prev.title,
        thumbnail: source.thumbnail ?? prev.thumbnail,
        durationSecs: source.durationSecs ?? prev.durationSecs,
      };
      if (
        merged.title === prev.title &&
        merged.thumbnail === prev.thumbnail &&
        merged.durationSecs === prev.durationSecs
      ) {
        return s; // nothing changed — avoid a needless re-render
      }
      const items = s.items.slice();
      items[idx] = merged;
      return { items };
    }),
  remove: (url) => set((s) => ({ items: s.items.filter((i) => i.url !== url) })),
  clear: () => set({ items: [] }),
  setActiveContext: (tabId, pageUrl) =>
    set({ activeTabId: tabId, activePageUrl: pageUrl }),
}));

/** An HLS per-rendition media sub-playlist (`index-v1-a1.m3u8`) — redundant
 * when its `master.m3u8` is also present. */
const HLS_SUBPLAYLIST_RE = /\/index-[va]\d+(-[va]\d+)*\.m3u8/i;

/** Items the volatile widget should actually show: unscoped ones (not from
 * the extension — always shown) plus those matching the active tab+page. Then
 * collapse redundant HLS sub-playlists: within a page, if a `master.m3u8` was
 * also detected, hide its `index-*.m3u8` renditions (yt-dlp reads them from the
 * master anyway). Done display-side and only when a master exists, so a stream
 * served *directly* as a media playlist (no master — some vmXXX CDNs) is never
 * hidden. */
export function visibleDetections(s: DetectionsState): MediaSourceDto[] {
  const scoped = s.items.filter((i) => {
    if (i.sourceTabId == null) return true;
    return i.sourceTabId === s.activeTabId && i.pageUrl === s.activePageUrl;
  });
  const pagesWithMaster = new Set(
    scoped.filter((i) => /master\.m3u8/i.test(i.url)).map((i) => i.pageUrl ?? ""),
  );
  return scoped.filter(
    (i) =>
      !HLS_SUBPLAYLIST_RE.test(i.url) || !pagesWithMaster.has(i.pageUrl ?? ""),
  );
}
