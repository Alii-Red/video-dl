import { useMemo, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  AlertTriangle,
  FolderOpen,
  History as HistoryIcon,
  RotateCw,
  Search,
  Trash2,
} from "lucide-react";
import { commands } from "../../lib/ipc";
import { useDownloadsStore } from "../../stores/downloadsStore";
import { useStartDownload } from "../downloads/hooks";
import { Card } from "../../components/ui/Card";
import { Button } from "../../components/ui/Button";
import type { HistoryEntryDto } from "../../types/HistoryEntryDto";

function formatBytes(bytes: bigint | null): string {
  if (bytes == null) return "—";
  const n = Number(bytes);
  if (n < 1024) return `${n} o`;
  const units = ["Ko", "Mo", "Go"];
  let v = n / 1024;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i++;
  }
  return `${v.toFixed(1)} ${units[i]}`;
}

function matches(entry: HistoryEntryDto, query: string): boolean {
  const q = query.trim().toLowerCase();
  if (!q) return true;
  return (
    (entry.title ?? "").toLowerCase().includes(q) ||
    entry.sourceUrl.toLowerCase().includes(q)
  );
}

// A direct media file / manifest URL — these are typically signed and
// short-lived (KVS `get_file/…`, HLS/DASH manifests, pasted direct links), so
// the stored URL is dead by the time you'd re-download it (HTTP 410). Only a
// *page* URL (YouTube, X, a site page) stays re-resolvable, so we only offer
// "Retélécharger" for those.
function isRedownloadable(url: string): boolean {
  return !/\/get_file\/|\.(m3u8|mpd|mp4|m4v|webm|mov|ts|m4s)(\?|#|$)/i.test(url);
}

export function HistoryPage() {
  const qc = useQueryClient();
  const [query, setQuery] = useState("");
  const [redownloadError, setRedownloadError] = useState<string | null>(null);
  const addItem = useDownloadsStore((s) => s.add);
  const start = useStartDownload();

  const { data, isLoading, isError, error } = useQuery({
    queryKey: ["history"],
    queryFn: () => commands.listHistory(),
  });

  const deleteEntry = useMutation({
    mutationFn: (downloadId: string) => commands.deleteHistoryEntry(downloadId),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["history"] }),
  });

  const openFolder = useMutation({
    mutationFn: (path: string) => commands.openContainingFolder(path),
  });

  const entries = data ?? [];
  const filtered = useMemo(
    () => entries.filter((e) => matches(e, query)),
    [entries, query],
  );

  const onRedownload = async (entry: HistoryEntryDto) => {
    setRedownloadError(null);
    try {
      // Only offered for page URLs (see `isRedownloadable`), which yt-dlp
      // re-resolves itself — so we hand it straight to the pipeline without a
      // probe (a page URL isn't a media file; probing it would just fail).
      const res = await start.mutateAsync({
        url: entry.sourceUrl,
        providerId: "generic",
        format: "progressive_mp_4",
        title: entry.title,
        maxHeight: null,
        audioOnly: false,
        referer: null,
      });
      addItem({
        taskId: res.taskId,
        downloadId: res.downloadId,
        url: entry.sourceUrl,
        title: entry.title,
        state: "queued",
        percent: 0,
      });
    } catch (e) {
      setRedownloadError(
        e && typeof e === "object" && "message" in e
          ? String((e as { message: unknown }).message)
          : "Échec du re-téléchargement.",
      );
    }
  };

  if (isLoading) return <p className="text-sm text-neutral-400">Chargement…</p>;
  if (isError)
    return <p className="text-sm text-red-400">Erreur : {(error as Error).message}</p>;

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between gap-3">
        <h2 className="text-2xl font-semibold tracking-tight">Historique</h2>
        {entries.length > 0 && (
          <div className="relative w-64">
            <Search className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-neutral-500" />
            <input
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder="Rechercher…"
              className="w-full rounded-lg border border-neutral-700 bg-neutral-900 py-2 pl-9 pr-3 text-sm outline-none transition-colors placeholder:text-neutral-600 focus:border-accent-500"
            />
          </div>
        )}
      </div>

      {redownloadError && (
        <Card className="flex items-start gap-2 border-red-900/70 bg-red-950/30 p-3 text-xs text-red-300">
          <AlertTriangle className="mt-0.5 h-3.5 w-3.5 shrink-0" />
          {redownloadError}
        </Card>
      )}

      {entries.length === 0 ? (
        <div className="flex flex-col items-center justify-center gap-3 py-20 text-center">
          <div className="flex h-14 w-14 items-center justify-center rounded-2xl bg-neutral-900 text-neutral-600">
            <HistoryIcon className="h-7 w-7" />
          </div>
          <p className="text-sm text-neutral-400">Aucun téléchargement terminé.</p>
        </div>
      ) : filtered.length === 0 ? (
        <p className="text-sm text-neutral-400">Aucun résultat pour « {query} ».</p>
      ) : (
        <ul className="space-y-2">
          {filtered.map((entry, i) => (
            <Card
              key={`${entry.downloadId}-${i}`}
              as="li"
              className="p-3.5 text-sm transition-colors hover:border-neutral-700"
            >
              <div className="flex items-center justify-between gap-3">
                <span className="flex min-w-0 items-center gap-2 font-medium">
                  <span className="h-1.5 w-1.5 shrink-0 rounded-full bg-emerald-500" />
                  <span className="truncate">{entry.title ?? entry.sourceUrl}</span>
                </span>
                <span className="shrink-0 text-xs text-neutral-500">
                  {new Date(entry.completedAt).toLocaleString()}
                </span>
              </div>
              <div className="mt-1 flex items-center justify-between gap-3 pl-3.5 text-xs text-neutral-500">
                <span className="truncate">{entry.finalPath ?? "—"}</span>
                <span className="shrink-0 tabular-nums">{formatBytes(entry.bytes)}</span>
              </div>
              <div className="mt-2.5 flex justify-end gap-2">
                {entry.finalPath && (
                  <Button
                    variant="secondary"
                    size="sm"
                    onClick={() => openFolder.mutate(entry.finalPath as string)}
                  >
                    <FolderOpen className="h-3.5 w-3.5" /> Ouvrir le dossier
                  </Button>
                )}
                {isRedownloadable(entry.sourceUrl) && (
                  <Button
                    variant="secondary"
                    size="sm"
                    onClick={() => void onRedownload(entry)}
                    disabled={start.isPending}
                  >
                    <RotateCw className="h-3.5 w-3.5" /> Retélécharger
                  </Button>
                )}
                <Button
                  variant="danger"
                  size="sm"
                  onClick={() => deleteEntry.mutate(entry.downloadId)}
                >
                  <Trash2 className="h-3.5 w-3.5" /> Supprimer
                </Button>
              </div>
            </Card>
          ))}
        </ul>
      )}
    </div>
  );
}
