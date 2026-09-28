import { create } from "zustand";
import type { TaskStateDto } from "../types/TaskStateDto";

export interface DownloadItem {
  taskId: string;
  downloadId: string;
  url: string;
  title: string | null;
  state: TaskStateDto;
  percent: number;
  /** Download speed in bytes/second, when yt-dlp reports it. */
  speedBps?: number | null;
  /** Estimated seconds remaining, when yt-dlp reports it. */
  etaSecs?: number | null;
  /** Short failure reason, set when the task transitions to `failed`. */
  error?: string | null;
}

interface DownloadsState {
  items: Record<string, DownloadItem>;
  add: (item: DownloadItem) => void;
  setState: (taskId: string, state: TaskStateDto, error?: string | null) => void;
  setProgress: (
    taskId: string,
    percent: number,
    speedBps?: number | null,
    etaSecs?: number | null,
  ) => void;
  remove: (taskId: string) => void;
}

/** UI-only state for the active downloads panel, kept in sync by IPC events. */
export const useDownloadsStore = create<DownloadsState>((set) => ({
  items: {},
  add: (item) =>
    set((s) => ({ items: { ...s.items, [item.taskId]: item } })),
  setState: (taskId, state, error) =>
    set((s) =>
      s.items[taskId]
        ? {
            items: {
              ...s.items,
              [taskId]: {
                ...s.items[taskId],
                state,
                error: error ?? (state === "failed" ? s.items[taskId].error : null),
              },
            },
          }
        : s,
    ),
  setProgress: (taskId, percent, speedBps, etaSecs) =>
    set((s) =>
      s.items[taskId]
        ? {
            items: {
              ...s.items,
              [taskId]: { ...s.items[taskId], percent, speedBps, etaSecs },
            },
          }
        : s,
    ),
  remove: (taskId) =>
    set((s) => {
      const next = { ...s.items };
      delete next[taskId];
      return { items: next };
    }),
}));
