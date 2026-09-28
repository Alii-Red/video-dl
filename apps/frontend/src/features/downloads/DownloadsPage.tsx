import { useEffect } from "react";
import { useQuery } from "@tanstack/react-query";
import { useShallow } from "zustand/react/shallow";
import { AlertTriangle, DownloadCloud, Pause, Play, RotateCw, Trash2, X } from "lucide-react";
import { useDownloadsStore } from "../../stores/downloadsStore";
import { commands } from "../../lib/ipc";
import { ProgressBar } from "../../components/ui/ProgressBar";
import { StateBadge } from "../../components/ui/StateBadge";
import { Card } from "../../components/ui/Card";
import { IconButton } from "../../components/ui/Button";
import { formatEta, formatSpeed } from "../../lib/format";
import {
  useCancelDownload,
  useDownloadEvents,
  usePauseDownload,
  useRemoveDownload,
  useRetryDownload,
  useResumeDownload,
} from "./hooks";

export function DownloadsPage() {
  useDownloadEvents();
  // Object.values() allocates a new array every call; wrap with useShallow so
  // Zustand compares array *contents* instead of reference identity — without
  // this, useSyncExternalStore sees a "new" value on every check and loops.
  const items = useDownloadsStore(useShallow((s) => Object.values(s.items)));
  const add = useDownloadsStore((s) => s.add);
  const cancel = useCancelDownload();
  const pause = usePauseDownload();
  const resume = useResumeDownload();
  const removeDownload = useRemoveDownload();
  const retryDownload = useRetryDownload();

  // Hydrate from the backend on mount so downloads survive a UI reload.
  const { data: active } = useQuery({
    queryKey: ["active"],
    queryFn: () => commands.listActive(),
  });
  useEffect(() => {
    active?.forEach((view) =>
      add({
        taskId: view.task.id,
        downloadId: view.download.id,
        url: view.download.sourceUrl,
        title: view.download.title,
        state: view.task.state,
        percent: view.task.progress.percent,
      }),
    );
  }, [active, add]);

  if (items.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center gap-3 py-20 text-center">
        <div className="flex h-14 w-14 items-center justify-center rounded-2xl bg-neutral-900 text-neutral-600">
          <DownloadCloud className="h-7 w-7" />
        </div>
        <p className="text-sm text-neutral-400">
          Aucun téléchargement actif. Lance-en un depuis l'onglet Détection.
        </p>
      </div>
    );
  }

  return (
    <div className="space-y-4">
      <h2 className="text-2xl font-semibold tracking-tight">Téléchargements</h2>
      <ul className="space-y-2.5">
        {items.map((item) => (
          <Card key={item.taskId} as="li" className="space-y-2.5 p-4">
            <div className="flex items-center justify-between gap-3">
              <span className="truncate text-sm font-medium">
                {item.title ?? item.url}
              </span>
              <StateBadge state={item.state} />
            </div>
            {item.state === "failed" ? (
              <p className="flex items-start gap-2 rounded-lg bg-red-950/40 px-3 py-2 text-xs text-red-300">
                <AlertTriangle className="mt-0.5 h-3.5 w-3.5 shrink-0" />
                {item.error ?? "Le téléchargement a échoué."}
              </p>
            ) : (
              <ProgressBar percent={item.percent} />
            )}
            <div className="flex items-center justify-between">
              <span className="text-xs tabular-nums text-neutral-500">
                {item.state === "failed" || item.state === "cancelled" ? (
                  ""
                ) : (
                  <>
                    {Math.round(item.percent)}%
                    {item.state === "running" &&
                      formatSpeed(item.speedBps) &&
                      ` · ${formatSpeed(item.speedBps)}`}
                    {item.state === "running" &&
                      formatEta(item.etaSecs) &&
                      ` · ${formatEta(item.etaSecs)} restant`}
                  </>
                )}
              </span>
              <div className="flex gap-1.5">
                {item.state === "failed" && (
                  <IconButton
                    variant="secondary"
                    aria-label="Réessayer"
                    title="Réessayer"
                    onClick={() => retryDownload.mutate(item.taskId)}
                  >
                    <RotateCw className="h-4 w-4" />
                  </IconButton>
                )}
                {(item.state === "failed" || item.state === "cancelled") && (
                  <IconButton
                    variant="secondary"
                    aria-label="Retirer"
                    title="Retirer"
                    onClick={() => removeDownload.mutate(item.taskId)}
                  >
                    <Trash2 className="h-4 w-4" />
                  </IconButton>
                )}
                {item.state === "running" && (
                  <IconButton
                    variant="secondary"
                    aria-label="Pause"
                    title="Pause"
                    onClick={() => pause.mutate(item.taskId)}
                  >
                    <Pause className="h-4 w-4" />
                  </IconButton>
                )}
                {item.state === "paused" && (
                  <IconButton
                    variant="secondary"
                    aria-label="Reprendre"
                    title="Reprendre"
                    onClick={() => resume.mutate(item.taskId)}
                  >
                    <Play className="h-4 w-4" />
                  </IconButton>
                )}
                {(item.state === "running" ||
                  item.state === "paused" ||
                  item.state === "queued") && (
                  <IconButton
                    variant="danger"
                    aria-label="Annuler"
                    title="Annuler"
                    onClick={() => cancel.mutate(item.taskId)}
                  >
                    <X className="h-4 w-4" />
                  </IconButton>
                )}
              </div>
            </div>
          </Card>
        ))}
      </ul>
    </div>
  );
}
