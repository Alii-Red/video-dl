import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { useDownloadsStore } from "../../stores/downloadsStore";
import type { HistoryEntryDto } from "../../types/HistoryEntryDto";

const listHistory = vi.fn();
const deleteHistoryEntry = vi.fn().mockResolvedValue(undefined);
const openContainingFolder = vi.fn().mockResolvedValue(undefined);
const probeUrl = vi.fn();
const startDownload = vi.fn().mockResolvedValue({ downloadId: "d1", taskId: "t1" });

vi.mock("../../lib/ipc", () => ({
  commands: {
    listHistory: (...a: unknown[]) => listHistory(...a),
    deleteHistoryEntry: (...a: unknown[]) => deleteHistoryEntry(...a),
    openContainingFolder: (...a: unknown[]) => openContainingFolder(...a),
    probeUrl: (...a: unknown[]) => probeUrl(...a),
    startDownload: (...a: unknown[]) => startDownload(...a),
  },
  events: {},
}));

const { HistoryPage } = await import("./HistoryPage");

function entry(overrides: Partial<HistoryEntryDto> = {}): HistoryEntryDto {
  return {
    downloadId: "d1",
    title: "Episode 1",
    sourceUrl: "https://cdn/x.m3u8",
    finalPath: "C:/Downloads/Episode 1.mp4",
    bytes: 1024n,
    completedAt: new Date().toISOString(),
    ...overrides,
  };
}

function renderPage() {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={queryClient}>
      <HistoryPage />
    </QueryClientProvider>,
  );
}

beforeEach(() => {
  useDownloadsStore.setState({ items: {} });
  vi.clearAllMocks();
  deleteHistoryEntry.mockResolvedValue(undefined);
  openContainingFolder.mockResolvedValue(undefined);
  startDownload.mockResolvedValue({ downloadId: "d1", taskId: "t1" });
});

afterEach(() => cleanup());

describe("HistoryPage", () => {
  it("shows the empty state with nothing in history", async () => {
    listHistory.mockResolvedValue([]);
    renderPage();
    expect(
      await screen.findByText("Aucun téléchargement terminé."),
    ).toBeInTheDocument();
  });

  it("filters entries by the search box", async () => {
    listHistory.mockResolvedValue([
      entry({ downloadId: "d1", title: "Dragon Ball" }),
      entry({ downloadId: "d2", title: "One Piece" }),
    ]);
    renderPage();
    await screen.findByText("Dragon Ball");
    expect(screen.getByText("One Piece")).toBeInTheDocument();

    const user = userEvent.setup();
    await user.type(screen.getByPlaceholderText("Rechercher…"), "dragon");

    expect(screen.getByText("Dragon Ball")).toBeInTheDocument();
    expect(screen.queryByText("One Piece")).not.toBeInTheDocument();
  });

  it("deletes an entry and refetches the list", async () => {
    listHistory.mockResolvedValueOnce([entry()]).mockResolvedValueOnce([]);
    renderPage();
    await screen.findByText("Episode 1");

    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: "Supprimer" }));

    expect(deleteHistoryEntry).toHaveBeenCalledWith("d1");
    await waitFor(() =>
      expect(screen.getByText("Aucun téléchargement terminé.")).toBeInTheDocument(),
    );
  });

  it("opens the containing folder", async () => {
    listHistory.mockResolvedValue([entry()]);
    renderPage();
    await screen.findByText("Episode 1");

    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: "Ouvrir le dossier" }));

    expect(openContainingFolder).toHaveBeenCalledWith("C:/Downloads/Episode 1.mp4");
  });

  it("hides the folder button when there's no final path", async () => {
    listHistory.mockResolvedValue([entry({ finalPath: null })]);
    renderPage();
    await screen.findByText("Episode 1");
    expect(screen.queryByRole("button", { name: "Ouvrir le dossier" })).not.toBeInTheDocument();
  });

  it("re-queues a page URL directly, without probing (which would 410 on an expired direct link)", async () => {
    listHistory.mockResolvedValue([
      entry({ title: "A YouTube Video", sourceUrl: "https://youtube.com/watch?v=x" }),
    ]);
    renderPage();
    await screen.findByText("A YouTube Video");

    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: "Retélécharger" }));

    await waitFor(() => expect(startDownload).toHaveBeenCalled());
    expect(probeUrl).not.toHaveBeenCalled();
    expect(startDownload).toHaveBeenCalledWith(
      expect.objectContaining({ url: "https://youtube.com/watch?v=x" }),
    );
    expect(useDownloadsStore.getState().items["t1"]).toBeDefined();
  });

  it("hides Retélécharger for a direct/expirable media URL", async () => {
    listHistory.mockResolvedValue([entry({ sourceUrl: "https://cdn/x.m3u8" })]);
    renderPage();
    await screen.findByText("Episode 1");
    expect(
      screen.queryByRole("button", { name: "Retélécharger" }),
    ).not.toBeInTheDocument();
    // The other actions remain.
    expect(screen.getByRole("button", { name: "Supprimer" })).toBeInTheDocument();
  });
});
