import { useEffect } from "react";
import { useMutation } from "@tanstack/react-query";
import {
  isPermissionGranted,
  requestPermission,
  sendNotification,
} from "@tauri-apps/plugin-notification";
import { commands, events } from "../../lib/ipc";
import { useDownloadsStore } from "../../stores/downloadsStore";
import type { StartDownloadArgs } from "../../types/StartDownloadArgs";

// Fire a desktop notification once per finished download (success/failure), so
// the user hears about it even when the window is minimised to the tray. Deduped
// per task since the terminal state event could, in theory, arrive more than
// once.
const notifiedTasks = new Set<string>();

async function notifyDownloadDone(
  title: string | null,
  ok: boolean,
  error: string | null,
) {
  try {
    let granted = await isPermissionGranted();
    if (!granted) granted = (await requestPermission()) === "granted";
    if (!granted) return;
    const name = title ?? "Téléchargement";
    sendNotification(
      ok
        ? { title: "Téléchargement terminé", body: name }
        : { title: "Échec du téléchargement", body: error ? `${name} — ${error}` : name },
    );
  } catch {
    // Notifications are best-effort; never let them break the event flow.
  }
}

export function useStartDownload() {
  return useMutation({
    mutationFn: (args: StartDownloadArgs) => commands.startDownload(args),
  });
}

export function useCancelDownload() {
  return useMutation({ mutationFn: (taskId: string) => commands.cancelDownload(taskId) });
}

export function usePauseDownload() {
  return useMutation({ mutationFn: (taskId: string) => commands.pauseDownload(taskId) });
}

export function useResumeDownload() {
  return useMutation({ mutationFn: (taskId: string) => commands.resumeDownload(taskId) });
}

/** Re-queue a failed download (fresh yt-dlp run — fixes YouTube's transient
 * 403s). The state flips back via the task-state event. */
export function useRetryDownload() {
  return useMutation({ mutationFn: (taskId: string) => commands.retryDownload(taskId) });
}

/** Dismisses a failed/cancelled entry — deletes it backend-side (not just
 * from the local store) so it doesn't reappear on the next reload. */
export function useRemoveDownload() {
  const remove = useDownloadsStore((s) => s.remove);
  return useMutation({
    mutationFn: async (taskId: string) => {
      await commands.removeDownload(taskId);
      remove(taskId);
    },
  });
}

/** Subscribe to backend progress/state events and mirror them into the store. */
export function useDownloadEvents() {
  const setProgress = useDownloadsStore((s) => s.setProgress);
  const setState = useDownloadsStore((s) => s.setState);

  useEffect(() => {
    const unlisteners = [
      events.onProgress((p) =>
        setProgress(
          p.taskId,
          p.progress.percent,
          p.progress.speedBps == null ? null : Number(p.progress.speedBps),
          p.progress.etaSecs == null ? null : Number(p.progress.etaSecs),
        ),
      ),
      events.onTaskState((p) => {
        setState(p.taskId, p.state, p.error);
        if (
          (p.state === "completed" || p.state === "failed") &&
          !notifiedTasks.has(p.taskId)
        ) {
          notifiedTasks.add(p.taskId);
          const item = useDownloadsStore.getState().items[p.taskId];
          void notifyDownloadDone(
            item?.title ?? null,
            p.state === "completed",
            p.error ?? null,
          );
        }
      }),
    ];
    return () => {
      for (const u of unlisteners) {
        void u.then((fn) => fn());
      }
    };
  }, [setProgress, setState]);
}
