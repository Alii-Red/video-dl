import { beforeEach, describe, expect, it } from "vitest";
import { useDetectionsStore, visibleDetections } from "./detectionsStore";
import type { MediaSourceDto } from "../types/MediaSourceDto";

function src(over: Partial<MediaSourceDto> = {}): MediaSourceDto {
  return {
    url: "https://www.youtube.com/watch?v=abc",
    format: "progressive_mp_4" as MediaSourceDto["format"],
    title: null,
    qualities: [],
    protected: false,
    thumbnail: null,
    durationSecs: null,
    sourceTabId: 7,
    pageUrl: "https://www.youtube.com/watch?v=abc",
    ...over,
  };
}

describe("detectionsStore", () => {
  beforeEach(() => useDetectionsStore.setState({ items: [], activeTabId: null, activePageUrl: null }));

  it("adds a new source at the front", () => {
    useDetectionsStore.getState().add(src({ title: "First" }));
    expect(useDetectionsStore.getState().items).toHaveLength(1);
  });

  it("merges a fresher title into an existing same-URL detection (widget refresh)", () => {
    const add = () => useDetectionsStore.getState().add;
    add()(src({ title: "Historique de visionnage - YouTube" }));
    // A refresh re-emits the SAME url once document.title has caught up.
    add()(src({ title: "A dream...I saw a dream (subaru)" }));
    const items = useDetectionsStore.getState().items;
    expect(items).toHaveLength(1); // still deduped, not doubled
    expect(items[0].title).toBe("A dream...I saw a dream (subaru)");
  });

  it("does not wipe a good thumbnail when a re-emit omits it", () => {
    const add = () => useDetectionsStore.getState().add;
    add()(src({ title: "Old", thumbnail: "https://img/poster.jpg" }));
    add()(src({ title: "New", thumbnail: null }));
    const item = useDetectionsStore.getState().items[0];
    expect(item.title).toBe("New");
    expect(item.thumbnail).toBe("https://img/poster.jpg");
  });

  it("shows only the current episode's scoped detections, keeping others as history", () => {
    const store = useDetectionsStore.getState();
    store.add(src({ url: "https://cdn/ep271/master.m3u8", pageUrl: "https://a.to/dbz#vdl:Episode%20271", title: "271" }));
    store.add(src({ url: "https://cdn/ep272/master.m3u8", pageUrl: "https://a.to/dbz#vdl:Episode%20272", title: "272" }));
    useDetectionsStore.getState().setActiveContext(7, "https://a.to/dbz#vdl:Episode%20272");
    // Both kept in history (never trimmed), but only episode 272 is visible.
    expect(useDetectionsStore.getState().items).toHaveLength(2);
    expect(visibleDetections(useDetectionsStore.getState()).map((i) => i.title)).toEqual(["272"]);
  });

  it("hides HLS sub-playlists when a master is present for the same page", () => {
    const store = useDetectionsStore.getState();
    const page = "https://a.to/dbz#vdl:Episode%20273";
    store.add(src({ url: "https://cdn/x_,n,l,.urlset/master.m3u8", pageUrl: page, title: "master" }));
    store.add(src({ url: "https://cdn/x_n/index-v1-a1.m3u8", pageUrl: page, title: "sub" }));
    useDetectionsStore.getState().setActiveContext(7, page);
    expect(visibleDetections(useDetectionsStore.getState()).map((i) => i.title)).toEqual(["master"]);
  });

  it("keeps a sub-playlist when there is no master (served directly)", () => {
    const store = useDetectionsStore.getState();
    const page = "https://a.to/dbz#vdl:Episode%20274";
    store.add(src({ url: "https://cdn/x_n/index-v1-a1.m3u8", pageUrl: page, title: "onlysub" }));
    useDetectionsStore.getState().setActiveContext(7, page);
    expect(visibleDetections(useDetectionsStore.getState()).map((i) => i.title)).toEqual(["onlysub"]);
  });

  it("scopes extension detections to the active tab+page", () => {
    useDetectionsStore.getState().add(src({ title: "T" }));
    useDetectionsStore.setState({ activeTabId: 7, activePageUrl: "https://www.youtube.com/watch?v=abc" });
    expect(visibleDetections(useDetectionsStore.getState())).toHaveLength(1);
    useDetectionsStore.getState().setActiveContext(7, "https://www.youtube.com/feed/history");
    expect(visibleDetections(useDetectionsStore.getState())).toHaveLength(0);
  });
});
