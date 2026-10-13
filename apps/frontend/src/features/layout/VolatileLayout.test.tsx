import { act, cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { useDetectionsStore } from "../../stores/detectionsStore";
import { useDownloadsStore } from "../../stores/downloadsStore";
import type { MediaSourceDto } from "../../types/MediaSourceDto";

const startDownload = vi.fn().mockResolvedValue({ downloadId: "d1", taskId: "t1" });

vi.mock("../../lib/ipc", () => ({
  commands: {
    listActive: vi.fn().mockResolvedValue([]),
    startDownload: (...a: unknown[]) => startDownload(...a),
    cancelDownload: vi.fn(),
    pauseDownload: vi.fn(),
    resumeDownload: vi.fn(),
    removeDownload: vi.fn().mockResolvedValue(undefined),
    retryDownload: vi.fn().mockResolvedValue(undefined),
    getSettings: vi.fn(),
    updateSettings: vi.fn(),
    extensionConnected: vi.fn().mockResolvedValue(false),
    ffmpegIsFull: vi.fn().mockResolvedValue(true),
    openDownloadFolder: vi.fn().mockResolvedValue(undefined),
  },
  events: {
    onProgress: vi.fn().mockResolvedValue(() => {}),
    onTaskState: vi.fn().mockResolvedValue(() => {}),
    onMediaDetected: vi.fn().mockResolvedValue(() => {}),
  },
}));

// Keep the window/Tauri APIs out of jsdom.
vi.mock("../window/useWindowMode", () => ({
  toggleWindowMode: vi.fn(),
  useWindowModeController: () => {},
}));

const { VolatileLayout } = await import("./VolatileLayout");

function source(url: string, title: string): MediaSourceDto {
  return {
    url,
    format: "hls",
    title,
    qualities: [],
    protected: false,
    thumbnail: null,
    durationSecs: null,
    sourceTabId: null,
    pageUrl: null,
  };
}

function renderWidget() {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={queryClient}>
      <VolatileLayout />
    </QueryClientProvider>,
  );
}

beforeEach(() => {
  useDetectionsStore.setState({
    items: [],
    activeTabId: null,
    activePageUrl: null,
  });
  useDownloadsStore.setState({ items: {} });
  vi.clearAllMocks();
});

afterEach(() => cleanup());

describe("VolatileLayout", () => {
  it("shows the empty state with nothing detected", async () => {
    renderWidget();
    expect(
      await screen.findByText(/il apparaîtra ici/i),
    ).toBeInTheDocument();
  });

  it("lists a detection and downloads it on click, moving it to active downloads", async () => {
    act(() => {
      useDetectionsStore.getState().add(source("https://cdn/x.m3u8", "Episode 1"));
    });

    renderWidget();
    expect(await screen.findByDisplayValue("Episode 1")).toBeInTheDocument();

    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: "Télécharger" }));

    // Started, removed from detections, present in the downloads store.
    expect(startDownload).toHaveBeenCalledWith(
      expect.objectContaining({ url: "https://cdn/x.m3u8", format: "hls" }),
    );
    expect(useDetectionsStore.getState().items).toHaveLength(0);
    expect(useDownloadsStore.getState().items["t1"]).toBeDefined();
    // Default quality = auto → no height cap, not audio-only.
    expect(startDownload).toHaveBeenCalledWith(
      expect.objectContaining({ maxHeight: null, audioOnly: false }),
    );
  });

  it("passes the chosen quality to the download", async () => {
    act(() => {
      useDetectionsStore.getState().add(source("https://cdn/x.m3u8", "Episode 1"));
    });
    renderWidget();
    await screen.findByDisplayValue("Episode 1");

    const user = userEvent.setup();
    await user.selectOptions(screen.getByRole("combobox", { name: "Qualité" }), "720");
    await user.click(screen.getByRole("button", { name: "Télécharger" }));

    expect(startDownload).toHaveBeenCalledWith(
      expect.objectContaining({ maxHeight: 720, audioOnly: false }),
    );
  });

  it("passes audio-only when Audio is chosen", async () => {
    act(() => {
      useDetectionsStore.getState().add(source("https://cdn/x.m3u8", "Episode 1"));
    });
    renderWidget();
    await screen.findByDisplayValue("Episode 1");

    const user = userEvent.setup();
    await user.selectOptions(screen.getByRole("combobox", { name: "Qualité" }), "audio");
    await user.click(screen.getByRole("button", { name: "Télécharger" }));

    expect(startDownload).toHaveBeenCalledWith(
      expect.objectContaining({ maxHeight: null, audioOnly: true }),
    );
  });

  it("dedupes detections by URL in the store", () => {
    act(() => {
      useDetectionsStore.getState().add(source("https://cdn/x.m3u8", "A"));
      useDetectionsStore.getState().add(source("https://cdn/x.m3u8", "A again"));
      useDetectionsStore.getState().add(source("https://cdn/y.m3u8", "B"));
    });
    expect(useDetectionsStore.getState().items).toHaveLength(2);
  });

  it("shows a generic format icon when no thumbnail is available", async () => {
    act(() => {
      useDetectionsStore.getState().add(source("https://cdn/x.m3u8", "No poster"));
    });
    renderWidget();
    await screen.findByDisplayValue("No poster");
    expect(screen.queryByRole("img")).not.toBeInTheDocument();
    expect(screen.getByText("🎞️")).toBeInTheDocument();
  });

  it("renders the poster image and formatted duration when available", async () => {
    act(() => {
      useDetectionsStore.getState().add({
        ...source("https://cdn/y.m3u8", "With poster"),
        thumbnail: "https://cdn/poster.jpg",
        durationSecs: 6844, // 1:54:04
      });
    });
    const { container } = renderWidget();
    await screen.findByDisplayValue("With poster");
    // jsdom has no real network stack, so the <img> may fire onError (and
    // get hidden) before this assertion runs — query it directly rather than
    // via getByRole, which excludes hidden elements.
    expect(container.querySelector("img")).toHaveAttribute(
      "src",
      "https://cdn/poster.jpg",
    );
    expect(screen.getByText("1:54:04")).toBeInTheDocument();
  });

  it("only shows detections matching the active tab/page, keeping the rest as history", async () => {
    act(() => {
      useDetectionsStore.getState().add({
        ...source("https://cdn/ep15.m3u8", "Episode 15"),
        sourceTabId: 7,
        pageUrl: "https://site/ep15",
      });
      useDetectionsStore.getState().add({
        ...source("https://cdn/ep16.m3u8", "Episode 16"),
        sourceTabId: 7,
        pageUrl: "https://site/ep16",
      });
      useDetectionsStore.getState().setActiveContext(7, "https://site/ep16");
    });

    renderWidget();
    expect(await screen.findByDisplayValue("Episode 16")).toBeInTheDocument();
    expect(screen.queryByDisplayValue("Episode 15")).not.toBeInTheDocument();
    // Not deleted — just hidden from the current view.
    expect(useDetectionsStore.getState().items).toHaveLength(2);
  });

  it("navigating to a page with nothing detected yet shows the empty state", async () => {
    act(() => {
      useDetectionsStore.getState().add({
        ...source("https://cdn/ep15.m3u8", "Episode 15"),
        sourceTabId: 7,
        pageUrl: "https://site/ep15",
      });
      useDetectionsStore.getState().setActiveContext(7, "https://site/ep16");
    });

    renderWidget();
    expect(
      await screen.findByText(/il apparaîtra ici/i),
    ).toBeInTheDocument();
  });

  it("always shows detections with no tab context (non-extension sources)", async () => {
    act(() => {
      useDetectionsStore.getState().add(source("https://cdn/manual.mp4", "Manual"));
      useDetectionsStore.getState().setActiveContext(7, "https://site/ep16");
    });

    renderWidget();
    expect(await screen.findByDisplayValue("Manual")).toBeInTheDocument();
  });

  it("shows the failure reason for a failed download and lets it be dismissed", async () => {
    act(() => {
      useDownloadsStore.setState({
        items: {
          t9: {
            taskId: "t9",
            downloadId: "d9",
            url: "https://youtube.com/watch?v=x",
            title: "A Video",
            state: "failed",
            percent: 0,
            error: "Video unavailable",
          },
        },
      });
    });

    renderWidget();
    expect(await screen.findByText("A Video")).toBeInTheDocument();
    expect(screen.getByText("Video unavailable")).toBeInTheDocument();

    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: "Retirer" }));
    expect(useDownloadsStore.getState().items).toEqual({});
  });
});
