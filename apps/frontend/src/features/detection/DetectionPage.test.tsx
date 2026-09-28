import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter } from "react-router-dom";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { useDownloadsStore } from "../../stores/downloadsStore";
import type { MediaSourceDto } from "../../types/MediaSourceDto";
import type { MediaDetectedPayload } from "../../types/MediaDetectedPayload";

const probeUrl = vi.fn();
const extractFromPage = vi.fn();
const openInteractiveExtraction = vi.fn();
const startDownload = vi.fn().mockResolvedValue({ downloadId: "d1", taskId: "t1" });
let mediaDetectedHandler: ((p: MediaDetectedPayload) => void) | null = null;

vi.mock("../../lib/ipc", () => ({
  commands: {
    probeUrl: (...args: unknown[]) => probeUrl(...args),
    extractFromPage: (...args: unknown[]) => extractFromPage(...args),
    openInteractiveExtraction: (...args: unknown[]) => openInteractiveExtraction(...args),
    startDownload: (...args: unknown[]) => startDownload(...args),
  },
  events: {
    onMediaDetected: (cb: (p: MediaDetectedPayload) => void) => {
      mediaDetectedHandler = cb;
      return Promise.resolve(() => {
        mediaDetectedHandler = null;
      });
    },
  },
}));

const { DetectionPage } = await import("./DetectionPage");

function mediaSource(url: string, title: string): MediaSourceDto {
  return {
    url,
    format: "progressive_mp_4",
    title,
    qualities: [],
    protected: false,
    thumbnail: null,
    durationSecs: null,
    sourceTabId: null,
    pageUrl: null,
  };
}

function renderPage() {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={queryClient}>
      <MemoryRouter>
        <DetectionPage />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

async function typeAndSubmit(url: string) {
  const user = userEvent.setup();
  await user.type(screen.getByPlaceholderText("https://…"), url);
  await user.click(screen.getByRole("button", { name: /analyser/i }));
}

beforeEach(() => {
  useDownloadsStore.setState({ items: {} });
  mediaDetectedHandler = null;
  vi.clearAllMocks();
});

afterEach(() => {
  cleanup();
});

describe("DetectionPage", () => {
  it("shows the source directly when the URL is a direct media link", async () => {
    probeUrl.mockResolvedValue(mediaSource("https://cdn.example.com/a.mp4", "Direct video"));

    renderPage();
    await typeAndSubmit("https://cdn.example.com/a.mp4");

    expect(await screen.findByText("Direct video")).toBeInTheDocument();
    expect(extractFromPage).not.toHaveBeenCalled();
  });

  it("falls back to silent page extraction when probe reports the wrong media type", async () => {
    probeUrl.mockRejectedValue({ kind: "probe", message: "unrecognized media type" });
    extractFromPage.mockResolvedValue([
      mediaSource("https://cdn.example.com/found1.m3u8", "Found stream"),
    ]);

    renderPage();
    await typeAndSubmit("https://site.example.com/watch");

    expect(await screen.findByText("Found stream")).toBeInTheDocument();
    expect(extractFromPage).toHaveBeenCalledWith("https://site.example.com/watch");
  });

  it("does not attempt page extraction for a non-probe error (e.g. invalid URL)", async () => {
    probeUrl.mockRejectedValue({ kind: "validation", message: "invalid url" });

    renderPage();
    await typeAndSubmit("not-a-url");

    await waitFor(() => expect(probeUrl).toHaveBeenCalled());
    expect(extractFromPage).not.toHaveBeenCalled();
  });

  it("offers interactive extraction when the silent pass finds nothing", async () => {
    probeUrl.mockRejectedValue({ kind: "probe", message: "unrecognized media type" });
    extractFromPage.mockResolvedValue([]);

    renderPage();
    await typeAndSubmit("https://site.example.com/watch");

    expect(
      await screen.findByText("Aucun flux détecté automatiquement sur cette page."),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: /ouvrir la page pour extraction interactive/i }),
    ).toBeInTheDocument();
  });

  it("streams live detections during an interactive session without looping", async () => {
    probeUrl.mockRejectedValue({ kind: "probe", message: "unrecognized media type" });
    extractFromPage.mockResolvedValue([]);
    let resolveInteractive: () => void = () => {};
    openInteractiveExtraction.mockImplementation(
      () => new Promise<void>((resolve) => (resolveInteractive = resolve)),
    );

    renderPage();
    await typeAndSubmit("https://site.example.com/watch");
    await screen.findByRole("button", { name: /ouvrir la page pour extraction interactive/i });

    const user = userEvent.setup();
    await user.click(
      screen.getByRole("button", { name: /ouvrir la page pour extraction interactive/i }),
    );

    expect(await screen.findByText(/fenêtre d'extraction ouverte/i)).toBeInTheDocument();
    expect(mediaDetectedHandler).not.toBeNull();

    // Simulate the backend emitting media://detected twice, once with a
    // duplicate URL that must not produce a second card.
    act(() => {
      mediaDetectedHandler!({
        source: mediaSource("https://cdn.example.com/live1.m3u8", "Live find 1"),
      });
    });
    act(() => {
      mediaDetectedHandler!({
        source: mediaSource("https://cdn.example.com/live1.m3u8", "Live find 1 dup"),
      });
    });
    act(() => {
      mediaDetectedHandler!({
        source: mediaSource("https://cdn.example.com/live2.m3u8", "Live find 2"),
      });
    });

    expect(await screen.findByText("Live find 1")).toBeInTheDocument();
    expect(await screen.findByText("Live find 2")).toBeInTheDocument();
    expect(screen.queryByText("Live find 1 dup")).not.toBeInTheDocument();

    resolveInteractive();
    await waitFor(() =>
      expect(
        screen.queryByText(/fenêtre d'extraction ouverte/i),
      ).not.toBeInTheDocument(),
    );
  });
});
