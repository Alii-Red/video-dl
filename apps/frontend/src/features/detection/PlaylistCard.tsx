import { useState } from "react";
import { Download, ListVideo, Loader2 } from "lucide-react";
import { Button } from "../../components/ui/Button";
import { Card } from "../../components/ui/Card";
import { formatEta } from "../../lib/format";
import type { PlaylistDto } from "../../types/PlaylistDto";
import type { PlaylistEntryDto } from "../../types/PlaylistEntryDto";

const QUALITIES: { label: string; maxHeight: number | null; audioOnly: boolean }[] = [
  { label: "Meilleure", maxHeight: null, audioOnly: false },
  { label: "1080p", maxHeight: 1080, audioOnly: false },
  { label: "720p", maxHeight: 720, audioOnly: false },
  { label: "480p", maxHeight: 480, audioOnly: false },
  { label: "Audio", maxHeight: null, audioOnly: true },
];

type Props = {
  playlist: PlaylistDto;
  busy: boolean;
  onDownload: (entries: PlaylistEntryDto[], maxHeight: number | null, audioOnly: boolean) => void;
  /** Present when the pasted URL also points at one specific video. */
  onSingleVideo?: () => void;
};

/** A resolved playlist: pick entries + quality, then queue them all at once. */
export function PlaylistCard({ playlist, busy, onDownload, onSingleVideo }: Props) {
  const [selected, setSelected] = useState<Set<string>>(
    () => new Set(playlist.entries.map((e) => e.url)),
  );
  const [quality, setQuality] = useState(0);

  const allSelected = selected.size === playlist.entries.length;
  const toggle = (url: string) =>
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(url)) next.delete(url);
      else next.add(url);
      return next;
    });

  const submit = () => {
    const q = QUALITIES[quality];
    onDownload(
      playlist.entries.filter((e) => selected.has(e.url)),
      q.maxHeight,
      q.audioOnly,
    );
  };

  return (
    <Card className="animate-fade-in-up space-y-4 p-4">
      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0">
          <div className="flex items-center gap-2 truncate font-medium">
            <ListVideo className="h-4 w-4 shrink-0 text-accent-400" />
            <span className="truncate">{playlist.title}</span>
          </div>
          <div className="mt-0.5 text-xs text-neutral-400">
            Playlist · {playlist.entries.length} vidéo{playlist.entries.length > 1 ? "s" : ""}
          </div>
        </div>
        <button
          type="button"
          className="shrink-0 text-xs text-accent-400 hover:text-accent-300"
          onClick={() =>
            setSelected(allSelected ? new Set() : new Set(playlist.entries.map((e) => e.url)))
          }
        >
          {allSelected ? "Tout décocher" : "Tout cocher"}
        </button>
      </div>

      <ul className="max-h-72 space-y-0.5 overflow-y-auto pr-1 text-sm">
        {playlist.entries.map((entry, i) => (
          <li key={entry.url}>
            <label className="flex cursor-pointer items-center gap-3 rounded-md px-2 py-1.5 hover:bg-neutral-800/60">
              <input
                type="checkbox"
                checked={selected.has(entry.url)}
                onChange={() => toggle(entry.url)}
                className="accent-accent-500"
              />
              <span className="w-6 shrink-0 text-right text-xs text-neutral-500">{i + 1}</span>
              <span className="min-w-0 flex-1 truncate text-neutral-200">
                {entry.title ?? entry.url}
              </span>
              {entry.durationSecs != null && (
                <span className="shrink-0 text-xs text-neutral-500">
                  {formatEta(entry.durationSecs)}
                </span>
              )}
            </label>
          </li>
        ))}
      </ul>

      <div className="flex flex-wrap items-center gap-2">
        <select
          aria-label="Qualité"
          value={quality}
          onChange={(e) => setQuality(Number(e.target.value))}
          className="rounded-lg border border-neutral-700 bg-neutral-900 px-2 py-2 text-sm text-neutral-200 outline-none focus:border-accent-500"
        >
          {QUALITIES.map((q, i) => (
            <option key={q.label} value={i}>
              {q.label}
            </option>
          ))}
        </select>
        <Button onClick={submit} disabled={busy || selected.size === 0}>
          {busy ? <Loader2 className="h-4 w-4 animate-spin" /> : <Download className="h-4 w-4" />}
          Télécharger {selected.size} vidéo{selected.size > 1 ? "s" : ""}
        </Button>
        {onSingleVideo && (
          <Button variant="secondary" onClick={onSingleVideo} disabled={busy}>
            Cette vidéo seulement
          </Button>
        )}
      </div>
    </Card>
  );
}
