import { act, cleanup, render, screen } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter } from "react-router-dom";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { useDownloadsStore } from "../../stores/downloadsStore";

// Mock the IPC boundary: no Tauri bridge exists in jsdom. `listActive` resolves
// to an empty list so the hydration effect is a no-op and doesn't interfere
// with the manually-seeded store state below.
vi.mock("../../lib/ipc", () => ({
  commands: {
    listActive: vi.fn().mockResolvedValue([]),
    startDownload: vi.fn(),
    cancelDownload: vi.fn(),
    pauseDownload: vi.fn(),
    resumeDownload: vi.fn(),
    removeDownload: vi.fn().mockResolvedValue(undefined),
    retryDownload: vi.fn().mockResolvedValue(undefined),
  },
  events: {
    onProgress: vi.fn().mockResolvedValue(() => {}),
    onTaskState: vi.fn().mockResolvedValue(() => {}),
    onMediaDetected: vi.fn().mockResolvedValue(() => {}),
  },
}));

// Imported after the mock above so DownloadsPage picks up the mocked module.
const { DownloadsPage } = await import("./DownloadsPage");

function renderPage() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(
    <QueryClientProvider client={queryClient}>
      <MemoryRouter>
        <DownloadsPage />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

const TASK_ID = "task-1";

beforeEach(() => {
  // Zustand stores are module-level singletons; reset between tests. Partial
  // merge (no `true` replace flag) so the action functions stay intact.
  useDownloadsStore.setState({ items: {} });
});

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe("DownloadsPage", () => {
  it("renders an active download and reflects live progress updates without looping", async () => {
    useDownloadsStore.getState().add({
      taskId: TASK_ID,
      downloadId: "dl-1",
      url: "https://example.com/video.mp4",
      title: "Test video",
      state: "running",
      percent: 10,
    });

    renderPage();

    expect(await screen.findByText("Test video")).toBeInTheDocument();
    expect(screen.getByText("10%")).toBeInTheDocument();

    // Simulate a burst of real-time progress events, as IPC would deliver
    // during an actual download. This is exactly the update pattern that
    // triggered "Maximum update depth exceeded" before the useShallow fix —
    // if the bug regresses, React throws synchronously inside act() and this
    // test fails immediately instead of hanging.
    for (const percent of [25, 40, 62, 81, 100]) {
      act(() => {
        useDownloadsStore.getState().setProgress(TASK_ID, percent);
      });
    }

    expect(screen.getByText("100%")).toBeInTheDocument();

    act(() => {
      useDownloadsStore.getState().setState(TASK_ID, "completed");
    });
    expect(await screen.findByText("Terminé")).toBeInTheDocument();
  });

  it("shows the empty state with no active downloads and does not throw", async () => {
    renderPage();
    expect(
      await screen.findByText(
        "Aucun téléchargement actif. Lance-en un depuis l'onglet Détection.",
      ),
    ).toBeInTheDocument();
  });

  it("survives repeated mount/unmount (tab switching) without an infinite loop", async () => {
    useDownloadsStore.getState().add({
      taskId: TASK_ID,
      downloadId: "dl-1",
      url: "https://example.com/video.mp4",
      title: "Test video",
      state: "running",
      percent: 0,
    });

    for (let i = 0; i < 5; i++) {
      const { unmount } = renderPage();
      expect(await screen.findByText("Test video")).toBeInTheDocument();
      act(() => {
        useDownloadsStore.getState().setProgress(TASK_ID, i * 20);
      });
      unmount();
    }
  });
});
