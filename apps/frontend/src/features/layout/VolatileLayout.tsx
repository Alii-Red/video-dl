import { useEffect, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useShallow } from "zustand/react/shallow";
import { commands } from "../../lib/ipc";
import { useDetectionsStore, visibleDetections } from "../../stores/detectionsStore";
import { useDownloadsStore } from "../../stores/downloadsStore";
import {
  useCancelDownload,
  useDownloadEvents,
  useRemoveDownload,
  useRetryDownload,
  useStartDownload,
} from "../downloads/hooks";
import { Download, Folder, RotateCw, X } from "lucide-react";
import { ProgressBar } from "../../components/ui/ProgressBar";
import { ModeToggle } from "../window/ModeToggle";
import { formatEta, formatSpeed } from "../../lib/format";
import type { MediaSourceDto } from "../../types/MediaSourceDto";

function formatDuration(totalSeconds: number): string {
  const h = Math.floor(totalSeconds / 3600);
  const m = Math.floor((totalSeconds % 3600) / 60);
  const s = Math.floor(totalSeconds % 60);
  const mm = String(m).padStart(2, "0");
  const ss = String(s).padStart(2, "0");
  return h > 0 ? `${h}:${mm}:${ss}` : `${mm}:${ss}`;
}

const FORMAT_ICON: Record<string, string> = {
  hls: "🎞️",
  dash: "🎞️",
  progressive_mp_4: "🎬",
  web_m: "🎬",
  mov: "🎬",
};

/** A per-download quality pick: a max height in pixels, "auto" (best
 * available), or "audio" (audio only). */
type QualityChoice = "auto" | "audio" | 1080 | 720 | 480;

const QUALITY_OPTIONS: { value: QualityChoice; label: string }[] = [
  { value: "auto", label: "Auto" },
  { value: 1080, label: "1080p" },
  { value: 720, label: "720p" },
  { value: 480, label: "480p" },
  { value: "audio", label: "Audio" },
];

function Thumbnail({ source }: { source: MediaSourceDto }) {
  if (!source.thumbnail) {
    return (
      <span className="flex h-10 w-16 shrink-0 items-center justify-center rounded bg-neutral-800 text-lg">
        {FORMAT_ICON[source.format] ?? "🎬"}
      </span>
    );
  }
  return (
    <div className="relative h-10 w-16 shrink-0 overflow-hidden rounded bg-neutral-800">
      <img
        src={source.thumbnail}
        alt=""
        loading="lazy"
        className="h-full w-full object-cover"
        onError={(e) => {
          // Broken poster URL (dead CDN link, CORS, ...): fall back to the
          // generic icon rather than showing a broken-image glyph.
          e.currentTarget.style.display = "none";
        }}
      />
      {source.durationSecs != null && (
        <span className="absolute bottom-0 right-0 rounded-tl bg-black/70 px-1 text-[9px] leading-tight text-neutral-200">
          {formatDuration(source.durationSecs)}
        </span>
      )}
    </div>
  );
}

/** One detected source: thumbnail, title, a per-download quality picker, and
 * the download button. Quality state is local so each row is independent. */
function DetectionRow({
  source,
  disabled,
  onDownload,
}: {
  source: MediaSourceDto;
  disabled: boolean;
  onDownload: (
    source: MediaSourceDto,
    quality: QualityChoice,
    title: string | null,
  ) => void;
}) {
  const [quality, setQuality] = useState<QualityChoice>("auto");
  // Editable file name (defaults to the detected title). Lets the user fix an
  // auto-title before downloading — handy on sites whose page title is generic.
  const [title, setTitle] = useState(source.title ?? "");
  return (
    <li className="flex items-center gap-2 rounded border border-neutral-800 bg-neutral-900 p-2">
      <Thumbnail source={source} />
      <div className="min-w-0 flex-1">
        <input
          value={title}
          onChange={(e) => setTitle(e.target.value)}
          placeholder={source.url}
          title="Nom du fichier (modifiable)"
          className="w-full truncate rounded border border-transparent bg-transparent text-xs text-neutral-100 outline-none hover:border-neutral-700 focus:border-accent-600 focus:bg-neutral-950"
        />
        <div className="text-[10px] uppercase tracking-wide text-neutral-500">
          {source.format}
          {source.protected ? " · protégé" : ""}
        </div>
      </div>
      <select
        value={String(quality)}
        onChange={(e) => {
          const v = e.target.value;
          setQuality(v === "auto" || v === "audio" ? v : (Number(v) as QualityChoice));
        }}
        disabled={source.protected}
        aria-label="Qualité"
        title="Qualité"
        className="shrink-0 rounded border border-neutral-700 bg-neutral-800 px-1 py-1 text-[10px] text-neutral-200 disabled:opacity-40"
      >
        {QUALITY_OPTIONS.map((o) => (
          <option key={o.label} value={String(o.value)}>
            {o.label}
          </option>
        ))}
      </select>
      <button
        onClick={() => onDownload(source, quality, title.trim() || null)}
        disabled={source.protected || disabled}
        title="Télécharger"
        aria-label="Télécharger"
        className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-accent-600 text-white transition-colors hover:bg-accent-500 active:bg-accent-700 disabled:cursor-not-allowed disabled:bg-neutral-700 disabled:opacity-50"
      >
        <Download className="h-4 w-4" />
      </button>
    </li>
  );
}

/**
 * Compact always-on-top widget (ADR 1.8): live detections at the top
 * (one-click download), active downloads below. No sidebar, no routing — the
 * essentials to keep visible over the browser while navigating.
 */
export function VolatileLayout() {
  useDownloadEvents();

  // The store keeps every detection ever seen (history, never trimmed); the
  // widget only shows the ones matching the extension's currently active
  // tab/page, so it tracks whatever's on screen in Opera GX right now.
  const detections = useDetectionsStore(useShallow(visibleDetections));
  const removeDetection = useDetectionsStore((s) => s.remove);
  const active = useDownloadsStore(useShallow((s) => Object.values(s.items)));
  const addItem = useDownloadsStore((s) => s.add);

  const start = useStartDownload();
  const cancel = useCancelDownload();
  const removeDownload = useRemoveDownload();
  const retryDownload = useRetryDownload();

  // Hydrate active downloads from the backend on mount (survives a reload).
  const { data: hydrated } = useQuery({
    queryKey: ["active"],
    queryFn: () => commands.listActive(),
  });
  useEffect(() => {
    hydrated?.forEach((view) =>
      addItem({
        taskId: view.task.id,
        downloadId: view.download.id,
        url: view.download.sourceUrl,
        title: view.download.title,
        state: view.task.state,
        percent: view.task.progress.percent,
      }),
    );
  }, [hydrated, addItem]);

  // Live extension-connection state → drives the header dot (green when the
  // Opera GX extension is talking to us, grey when not).
  const { data: connected } = useQuery({
    queryKey: ["ext-connected"],
    queryFn: () => commands.extensionConnected(),
    refetchInterval: 3000,
  });

  const onDownload = async (
    source: MediaSourceDto,
    quality: QualityChoice,
    title: string | null,
  ) => {
    const finalTitle = title ?? source.title;
    const res = await start.mutateAsync({
      url: source.url,
      providerId: "generic",
      format: source.format,
      title: finalTitle,
      maxHeight: typeof quality === "number" ? quality : null,
      audioOnly: quality === "audio",
      // The page this was detected on — sent as Referer for direct-file
      // hosts (KVS/tube players) that reject a hotlinked GET without it.
      referer: source.pageUrl,
    });
    addItem({
      taskId: res.taskId,
      downloadId: res.downloadId,
      url: source.url,
      title: finalTitle,
      state: "queued",
      percent: 0,
    });
    removeDetection(source.url);
  };

  const activeRunning = active.filter((i) => i.state !== "completed");
  const completed = active.filter((i) => i.state === "completed");
  const isEmpty =
    detections.length === 0 && activeRunning.length === 0 && completed.length === 0;

  return (
    <div className="flex h-screen flex-col bg-neutral-950 text-neutral-100">
      <header className="flex items-center justify-between border-b border-neutral-800 px-3 py-2">
        <div className="flex items-center gap-2">
          <span
            className={`h-2.5 w-2.5 rounded-full ${connected ? "bg-emerald-500" : "bg-neutral-600"}`}
            title={connected ? "Extension connectée" : "Extension non connectée"}
          />
          <span className="text-sm font-medium">Video DL</span>
          {detections.length > 0 && (
            <span className="rounded bg-emerald-900/60 px-1.5 text-xs text-emerald-300">
              {detections.length}
            </span>
          )}
        </div>
        <div className="flex items-center gap-1">
          <button
            onClick={() => void commands.rescanExtension()}
            title="Actualiser (re-scanner la page ouverte)"
            aria-label="Actualiser"
            className="flex h-7 w-7 items-center justify-center rounded-lg text-neutral-400 transition-colors hover:bg-neutral-800 hover:text-neutral-100"
          >
            <RotateCw className="h-4 w-4" />
          </button>
          <ModeToggle compact />
        </div>
      </header>

      <div className="flex-1 space-y-3 overflow-y-auto p-3">
        {isEmpty && (
          <p className="pt-6 text-center text-xs text-neutral-500">
            Navigue sur ton navigateur — les vidéos détectées apparaîtront ici.
          </p>
        )}

        {detections.length > 0 && (
          <ul className="space-y-1.5">
            {detections.map((source) => (
              <DetectionRow
                key={source.url}
                source={source}
                disabled={start.isPending}
                onDownload={onDownload}
              />
            ))}
          </ul>
        )}

        {activeRunning.length > 0 && (
          <ul className="space-y-1.5">
            {activeRunning.map((item) =>
              item.state === "failed" || item.state === "cancelled" ? (
                <li
                  key={item.taskId}
                  className="space-y-1 rounded border border-red-900/70 bg-red-950/30 p-2"
                >
                  <div className="flex items-center gap-2">
                    <span className="min-w-0 flex-1 truncate text-xs text-red-200">
                      {item.title ?? item.url}
                    </span>
                    <span className="shrink-0 text-[10px] font-medium uppercase text-red-400">
                      {item.state === "failed" ? "Échec" : "Annulé"}
                    </span>
                    {item.state === "failed" && (
                      <button
                        onClick={() => retryDownload.mutate(item.taskId)}
                        title="Réessayer"
                        aria-label="Réessayer"
                        className="flex h-5 w-5 shrink-0 items-center justify-center rounded text-neutral-300 hover:bg-neutral-800"
                      >
                        <RotateCw className="h-3.5 w-3.5" />
                      </button>
                    )}
                    <button
                      onClick={() => removeDownload.mutate(item.taskId)}
                      title="Retirer"
                      aria-label="Retirer"
                      className="flex h-5 w-5 shrink-0 items-center justify-center rounded text-red-400 hover:bg-red-950"
                    >
                      <X className="h-3.5 w-3.5" />
                    </button>
                  </div>
                  {item.state === "failed" && (
                    <p className="text-[10px] leading-tight text-red-300/80">
                      {item.error ?? "Le téléchargement a échoué."}
                    </p>
                  )}
                </li>
              ) : (
                <li
                  key={item.taskId}
                  className="space-y-1 rounded border border-neutral-800 bg-neutral-900 p-2"
                >
                  <div className="flex items-center gap-2">
                    <span className="min-w-0 flex-1 truncate text-xs">
                      {item.title ?? item.url}
                    </span>
                    <span className="shrink-0 text-[10px] text-neutral-500">
                      {Math.round(item.percent)}%
                    </span>
                    <button
                      onClick={() => cancel.mutate(item.taskId)}
                      title="Annuler"
                      aria-label="Annuler"
                      className="flex h-5 w-5 shrink-0 items-center justify-center rounded text-red-400 hover:bg-red-950"
                    >
                      <X className="h-3.5 w-3.5" />
                    </button>
                  </div>
                  <ProgressBar percent={item.percent} />
                  {item.state === "running" &&
                    (formatSpeed(item.speedBps) || formatEta(item.etaSecs)) && (
                      <div className="flex justify-between text-[9px] text-neutral-500">
                        <span>{formatSpeed(item.speedBps) ?? ""}</span>
                        <span>
                          {formatEta(item.etaSecs)
                            ? `${formatEta(item.etaSecs)} restant`
                            : ""}
                        </span>
                      </div>
                    )}
                </li>
              ),
            )}
          </ul>
        )}

        {completed.length > 0 && (
          <ul className="space-y-1.5">
            {completed.map((item) => (
              <li
                key={item.taskId}
                className="flex items-center gap-2 rounded border border-emerald-900/60 bg-emerald-950/20 p-2"
              >
                <span className="min-w-0 flex-1 truncate text-xs text-emerald-200">
                  {item.title ?? item.url}
                </span>
                <span className="shrink-0 text-[10px] font-medium uppercase text-emerald-400">
                  Terminé
                </span>
                <button
                  onClick={() => void commands.openDownloadFolder(item.downloadId)}
                  title="Ouvrir le dossier"
                  aria-label="Ouvrir le dossier"
                  className="flex h-5 w-5 shrink-0 items-center justify-center rounded text-neutral-300 hover:bg-neutral-800"
                >
                  <Folder className="h-3.5 w-3.5" />
                </button>
                <button
                  onClick={() => removeDownload.mutate(item.taskId)}
                  title="Retirer"
                  aria-label="Retirer"
                  className="flex h-5 w-5 shrink-0 items-center justify-center rounded text-neutral-400 hover:bg-neutral-800"
                >
                  <X className="h-3.5 w-3.5" />
                </button>
              </li>
            ))}
          </ul>
        )}
      </div>
    </div>
  );
}
