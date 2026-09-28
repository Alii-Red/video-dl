import { useEffect, useRef, useState, type FormEvent } from "react";
import { useMutation } from "@tanstack/react-query";
import {
  AlertTriangle,
  Download,
  Loader2,
  Lock,
  Search,
} from "lucide-react";
import { commands, events } from "../../lib/ipc";
import { useStartDownload } from "../downloads/hooks";
import { useDownloadsStore } from "../../stores/downloadsStore";
import { Button } from "../../components/ui/Button";
import { Card } from "../../components/ui/Card";
import type { MediaSourceDto } from "../../types/MediaSourceDto";
import type { IpcError } from "../../types/IpcError";

/** Dedup by URL, keeping the first-seen entry. */
function mergeSources(prev: MediaSourceDto[], next: MediaSourceDto[]): MediaSourceDto[] {
  const seen = new Set(prev.map((s) => s.url));
  const added = next.filter((s) => !seen.has(s.url));
  return [...prev, ...added];
}

export function DetectionPage() {
  const [url, setUrl] = useState("");
  const [sources, setSources] = useState<MediaSourceDto[]>([]);
  // Drives the status line under the form. "extracting" covers both the
  // automatic silent pass and the user-triggered interactive one.
  const [status, setStatus] = useState<
    "idle" | "probing" | "extracting" | "interactive-open" | "no-results"
  >("idle");
  const addItem = useDownloadsStore((s) => s.add);
  const [downloadError, setDownloadError] = useState<string | null>(null);
  // Mirror `sources` in a ref so async handlers (e.g. the interactive
  // extraction's `finally`) read the current value, not the one captured when
  // the handler was created.
  const sourcesRef = useRef(sources);
  sourcesRef.current = sources;

  const probe = useMutation({ mutationFn: (u: string) => commands.probeUrl(u) });
  const silentExtract = useMutation({ mutationFn: (u: string) => commands.extractFromPage(u) });
  const interactiveExtract = useMutation({
    mutationFn: (u: string) => commands.openInteractiveExtraction(u),
  });
  const start = useStartDownload();

  // While an interactive session is open, new sources stream in live via the
  // same media://detected channel used for direct-link probing.
  useEffect(() => {
    if (status !== "interactive-open") return;
    const unlisten = events.onMediaDetected((p) =>
      setSources((prev) => mergeSources(prev, [p.source])),
    );
    return () => {
      void unlisten.then((fn) => fn());
    };
  }, [status]);

  const onProbe = async (e: FormEvent) => {
    e.preventDefault();
    const trimmed = url.trim();
    if (!trimmed) return;

    setSources([]);
    setStatus("probing");

    try {
      const result = await probe.mutateAsync(trimmed);
      setSources([result]);
      setStatus("idle");
      return;
    } catch (err) {
      // Only a "wrong type of link" failure warrants trying page extraction —
      // a validation error (bad URL) or a network/ACL-level rejection (which
      // isn't shaped like our IpcError at all — e.g. a raw Tauri permission
      // denial string) should just surface. Logged raw so it's visible in
      // DevTools even when it doesn't match our expected error shape.
      console.error("[VDL] probe_url failed:", err);
      if ((err as IpcError)?.kind !== "probe") {
        setStatus("idle");
        return;
      }
    }

    // Direct link didn't work: this is likely a page URL. Try the fast,
    // invisible extraction pass automatically before asking the user to open
    // an interactive window.
    setStatus("extracting");
    try {
      const found = await silentExtract.mutateAsync(trimmed);
      if (found.length > 0) {
        setSources(found);
        setStatus("idle");
      } else {
        setStatus("no-results");
      }
    } catch {
      setStatus("no-results");
    }
  };

  const onOpenInteractive = async () => {
    const trimmed = url.trim();
    if (!trimmed) return;
    setStatus("interactive-open");
    try {
      await interactiveExtract.mutateAsync(trimmed);
    } finally {
      // Read the live value: sources may have streamed in during the session.
      setStatus(sourcesRef.current.length > 0 ? "idle" : "no-results");
    }
  };

  const onDownload = async (source: MediaSourceDto) => {
    setDownloadError(null);
    try {
      const res = await start.mutateAsync({
        url: source.url,
        providerId: "generic",
        format: source.format,
        title: source.title,
        // Standard-mode detection uses the global quality default (applied by
        // the planner); the per-download picker lives in the widget.
        maxHeight: null,
        audioOnly: false,
        // Manual/pasted URLs have no known origin page — direct-file hosts
        // that require a Referer (see the widget path) won't work from here.
        referer: source.pageUrl,
      });
      addItem({
        taskId: res.taskId,
        downloadId: res.downloadId,
        url: source.url,
        title: source.title,
        state: "queued",
        percent: 0,
      });
      // Once queued, drop the card so the list reflects what's still
      // actionable (consistent with the widget's behaviour).
      setSources((prev) => prev.filter((s) => s.url !== source.url));
    } catch (err) {
      // Previously swallowed silently — a rejected mutateAsync in an onClick
      // handler with no caller awaiting it just becomes an invisible
      // unhandled rejection. Surface it so "nothing happens" never happens.
      console.error("[VDL] start_download failed:", err);
      const message =
        err && typeof err === "object" && "message" in err
          ? String((err as { message: unknown }).message)
          : String(err);
      setDownloadError(message);
    }
  };

  const busy = status === "probing" || status === "extracting";

  return (
    <div className="max-w-2xl space-y-6">
      <div>
        <h2 className="text-2xl font-semibold tracking-tight">Détection</h2>
        <p className="mt-1 text-sm text-neutral-400">
          Colle une URL de média (MP4, WebM, MOV, HLS, DASH) ou une page qui en
          contient un.
        </p>
      </div>

      <form onSubmit={onProbe} className="flex gap-2">
        <div className="relative flex-1">
          <Search className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-neutral-500" />
          <input
            value={url}
            onChange={(e) => setUrl(e.target.value)}
            placeholder="https://…"
            className="w-full rounded-lg border border-neutral-700 bg-neutral-900 py-2.5 pl-9 pr-3 text-sm text-neutral-100 outline-none transition-colors placeholder:text-neutral-600 focus:border-accent-500"
          />
        </div>
        <Button type="submit" disabled={busy}>
          {busy ? (
            <Loader2 className="h-4 w-4 animate-spin" />
          ) : (
            <Search className="h-4 w-4" />
          )}
          {busy ? "Analyse…" : "Analyser"}
        </Button>
      </form>

      {downloadError && (
        <Card className="flex items-start gap-2 border-red-900/70 bg-red-950/30 p-3 text-sm text-red-300">
          <AlertTriangle className="mt-0.5 h-4 w-4 shrink-0" />
          <span>Échec du démarrage du téléchargement : {downloadError}</span>
        </Card>
      )}

      {status === "probing" && (
        <p className="flex items-center gap-2 text-sm text-neutral-400">
          <Loader2 className="h-4 w-4 animate-spin" /> Analyse du lien…
        </p>
      )}
      {status === "extracting" && (
        <p className="flex items-center gap-2 text-sm text-neutral-400">
          <Loader2 className="h-4 w-4 animate-spin" /> Lien direct non reconnu —
          analyse de la page en cours (jusqu'à 12s)…
        </p>
      )}
      {status === "interactive-open" && (
        <p className="text-sm text-neutral-400">
          Fenêtre d'extraction ouverte — interagis avec la page (ex. clique sur
          Lecture), les flux détectés apparaissent ici en direct. Ferme la
          fenêtre quand tu as terminé.
        </p>
      )}
      {status === "no-results" && (
        <Card className="space-y-3 border-amber-900/70 bg-amber-950/20 p-4 text-sm">
          <p className="flex items-center gap-2 text-amber-300">
            <AlertTriangle className="h-4 w-4" />
            Aucun flux détecté automatiquement sur cette page.
          </p>
          <Button variant="secondary" size="sm" onClick={onOpenInteractive}>
            Ouvrir la page pour extraction interactive
          </Button>
        </Card>
      )}

      {sources.length > 0 && (
        <ul className="space-y-3">
          {sources.map((source) => (
            <Card
              key={source.url}
              as="li"
              className="animate-fade-in-up space-y-4 p-4"
            >
              <div className="flex items-center justify-between gap-3">
                <div className="min-w-0">
                  <div className="truncate font-medium">
                    {source.title ?? "Média détecté"}
                  </div>
                  <div className="mt-0.5 text-xs font-medium uppercase tracking-wide text-accent-400">
                    {source.format}
                  </div>
                </div>
                {source.protected && (
                  <span className="flex shrink-0 items-center gap-1 rounded-md bg-red-900/50 px-2 py-1 text-xs text-red-300">
                    <Lock className="h-3 w-3" /> DRM
                  </span>
                )}
              </div>

              {source.qualities.length > 0 && (
                <ul className="space-y-1 text-sm text-neutral-300">
                  {source.qualities.map((q, i) => (
                    <li
                      key={i}
                      className="flex justify-between border-b border-neutral-800/70 py-1 last:border-0"
                    >
                      <span>{q.height ? `${q.height}p` : "audio/inconnu"}</span>
                      <span className="text-neutral-500">
                        {q.bitrate ? `${Math.round(Number(q.bitrate) / 1000)} kbps` : "—"}
                        {q.codec ? ` · ${q.codec}` : ""}
                      </span>
                    </li>
                  ))}
                </ul>
              )}

              <Button
                onClick={() => onDownload(source)}
                disabled={source.protected || start.isPending}
              >
                {source.protected ? (
                  <>
                    <Lock className="h-4 w-4" /> Non téléchargeable
                  </>
                ) : (
                  <>
                    <Download className="h-4 w-4" /> Télécharger
                  </>
                )}
              </Button>
            </Card>
          ))}
        </ul>
      )}
    </div>
  );
}
