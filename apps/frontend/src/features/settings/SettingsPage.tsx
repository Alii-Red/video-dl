import { useEffect, useState, type ReactNode } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  Check,
  Copy,
  FolderOpen,
  Info,
  RefreshCw,
  RotateCcw,
  Save,
  SlidersHorizontal,
  Wifi,
  Wrench,
} from "lucide-react";
import { commands } from "../../lib/ipc";
import { Card } from "../../components/ui/Card";
import { Button } from "../../components/ui/Button";
import type { SettingsDto } from "../../types/SettingsDto";

const heights = [null, 480, 720, 1080, 2160] as const;

/** Titled section card with an icon. */
function Section({
  icon: Icon,
  title,
  description,
  children,
}: {
  icon: typeof Info;
  title: string;
  description?: ReactNode;
  children: ReactNode;
}) {
  return (
    <Card className="space-y-4 p-5">
      <div className="flex items-center gap-2.5">
        <div className="flex h-8 w-8 items-center justify-center rounded-lg bg-neutral-800 text-accent-400">
          <Icon className="h-4 w-4" />
        </div>
        <h3 className="text-sm font-semibold text-neutral-100">{title}</h3>
      </div>
      {description && <p className="text-xs leading-relaxed text-neutral-500">{description}</p>}
      {children}
    </Card>
  );
}

/** Accessible on/off switch. */
function Toggle({
  checked,
  onChange,
  label,
}: {
  checked: boolean;
  onChange: (v: boolean) => void;
  label: string;
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      onClick={() => onChange(!checked)}
      className="flex w-full items-center justify-between gap-3 text-left"
    >
      <span className="text-sm text-neutral-300">{label}</span>
      <span
        className={`relative h-5 w-9 shrink-0 rounded-full transition-colors ${
          checked ? "bg-accent-600" : "bg-neutral-700"
        }`}
      >
        <span
          className={`absolute top-0.5 h-4 w-4 rounded-full bg-white transition-transform ${
            checked ? "translate-x-4" : "translate-x-0.5"
          }`}
        />
      </span>
    </button>
  );
}

const inputClass =
  "w-full rounded-lg border border-neutral-700 bg-neutral-900 px-3 py-2 text-sm text-neutral-100 outline-none transition-colors placeholder:text-neutral-600 focus:border-accent-500";

export function SettingsPage() {
  const qc = useQueryClient();
  const { data, isLoading } = useQuery({
    queryKey: ["settings"],
    queryFn: () => commands.getSettings(),
  });
  const [draft, setDraft] = useState<SettingsDto | null>(null);

  useEffect(() => {
    if (data) setDraft(data);
  }, [data]);

  const save = useMutation({
    mutationFn: (s: SettingsDto) => commands.updateSettings(s),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["settings"] }),
  });

  const updateTool = useMutation({ mutationFn: () => commands.updateYtdlp() });

  // Whether a full FFmpeg was found (can remux HLS audio). `false` → warn.
  const { data: ffmpegFull } = useQuery({
    queryKey: ["ffmpeg-full"],
    queryFn: () => commands.ffmpegIsFull(),
  });

  if (isLoading || !draft) {
    return <p className="text-sm text-neutral-400">Chargement…</p>;
  }

  const patch = (p: Partial<SettingsDto>) => setDraft({ ...draft, ...p });

  // Reset only the download *preferences* to their backend defaults; leave
  // machine-specific (downloadDir) and technical (extension/window) fields.
  const resetDefaults = () => {
    const next: SettingsDto = {
      ...draft,
      maxHeight: null,
      audioOnly: false,
      subtitles: false,
      concurrency: 2,
      rateLimitBps: null,
    };
    setDraft(next);
    save.mutate(next);
  };

  return (
    <div className="max-w-xl space-y-5">
      <h2 className="text-2xl font-semibold tracking-tight">Réglages</h2>

      {ffmpegFull === false && (
        <div className="rounded-lg border border-amber-800/60 bg-amber-950/30 p-3 text-xs leading-relaxed text-amber-200">
          <strong>FFmpeg minimal détecté.</strong> Les téléchargements HLS
          (anime-sama…) peuvent échouer au remux audio (« Postprocessing: Stream
          #0:1 »). Installe un FFmpeg complet — ou garde Shutter Encoder / CapCut
          installés : l'app détecte et utilise automatiquement le premier FFmpeg
          complet trouvé.
        </div>
      )}

      <Section icon={SlidersHorizontal} title="Téléchargement">
        <label className="block space-y-1.5">
          <span className="text-sm text-neutral-300">Dossier de téléchargement</span>
          <input
            value={draft.downloadDir}
            onChange={(e) => patch({ downloadDir: e.target.value })}
            placeholder="C:\Users\…\Downloads"
            className={inputClass}
          />
        </label>

        <label className="block space-y-1.5">
          <span className="text-sm text-neutral-300">Qualité maximale</span>
          <select
            value={draft.maxHeight ?? ""}
            onChange={(e) =>
              patch({ maxHeight: e.target.value ? Number(e.target.value) : null })
            }
            className={inputClass}
          >
            {heights.map((h) => (
              <option key={h ?? "best"} value={h ?? ""}>
                {h ? `${h}p` : "Meilleure disponible"}
              </option>
            ))}
          </select>
        </label>

        <Toggle
          label="Audio seulement"
          checked={draft.audioOnly}
          onChange={(v) => patch({ audioOnly: v })}
        />
        <Toggle
          label="Sous-titres"
          checked={draft.subtitles}
          onChange={(v) => patch({ subtitles: v })}
        />

        <label className="block space-y-1.5">
          <span className="flex items-center justify-between text-sm text-neutral-300">
            Téléchargements simultanés
            <span className="tabular-nums text-accent-400">{draft.concurrency}</span>
          </span>
          <input
            type="range"
            min={1}
            max={6}
            value={draft.concurrency}
            onChange={(e) => patch({ concurrency: Number(e.target.value) })}
            className="w-full accent-accent-500"
          />
        </label>
      </Section>

      <Section
        icon={Wifi}
        title="Détection live (extension navigateur)"
        description="Détecte automatiquement les vidéos pendant la navigation. L'extension se configure toute seule (le token est transmis automatiquement) — il reste juste à la charger une fois dans le navigateur."
      >
        <Toggle
          label="Activer le pont WebSocket local"
          checked={draft.extensionEnabled}
          onChange={(v) => patch({ extensionEnabled: v })}
        />

        <div className="rounded-lg border border-neutral-800 bg-neutral-900/50 p-3 text-sm text-neutral-300">
          <p className="mb-2 font-medium text-neutral-200">Installer l'extension (une fois) :</p>
          <ol className="list-inside list-decimal space-y-1 text-xs leading-relaxed text-neutral-400">
            <li>Ouvre <code className="text-neutral-300">opera://extensions</code> (ou <code className="text-neutral-300">chrome://extensions</code>) et active le <strong>Mode développeur</strong>.</li>
            <li>Clique <strong>« Charger une extension non empaquetée »</strong>.</li>
            <li>Sélectionne le dossier ci-dessous.</li>
          </ol>
          <Button
            type="button"
            variant="secondary"
            className="mt-3"
            onClick={() => void commands.revealExtensionFolder()}
          >
            <FolderOpen className="h-4 w-4" /> Ouvrir le dossier de l'extension
          </Button>
          <p className="mt-2 text-xs text-neutral-500">
            Le token se configure automatiquement — rien à copier-coller.
          </p>
        </div>

        <details className="text-xs text-neutral-500">
          <summary className="cursor-pointer select-none">Config manuelle (avancé)</summary>
          <div className="mt-2 space-y-2">
            <label className="block space-y-1.5">
              <span className="text-neutral-400">Port</span>
              <input
                type="number"
                value={draft.extensionPort}
                onChange={(e) => patch({ extensionPort: Number(e.target.value) || 8787 })}
                className={inputClass}
              />
            </label>
            <label className="block space-y-1.5">
              <span className="text-neutral-400">Token</span>
              <div className="flex gap-2">
                <input readOnly value={draft.extensionToken} className={`${inputClass} text-neutral-400`} />
                <Button
                  type="button"
                  variant="secondary"
                  onClick={() => navigator.clipboard.writeText(draft.extensionToken)}
                >
                  <Copy className="h-4 w-4" /> Copier
                </Button>
              </div>
            </label>
            <p>Redémarre l'app après un changement de port pour reconnecter le pont.</p>
          </div>
        </details>
      </Section>

      <Section
        icon={Wrench}
        title="Maintenance"
        description="YouTube change souvent son extraction. Si un téléchargement échoue, mets yt-dlp à jour ici (nécessite Python installé)."
      >
        <div className="flex flex-wrap items-center gap-3">
          <Button
            type="button"
            variant="secondary"
            onClick={() => updateTool.mutate()}
            disabled={updateTool.isPending}
          >
            <RefreshCw className={`h-4 w-4 ${updateTool.isPending ? "animate-spin" : ""}`} />
            {updateTool.isPending ? "Mise à jour…" : "Mettre à jour yt-dlp"}
          </Button>
          {updateTool.isSuccess && (
            <span className="flex items-center gap-1 text-sm text-emerald-400">
              <Check className="h-4 w-4" /> À jour : {updateTool.data}
            </span>
          )}
          {updateTool.isError && (
            <span className="text-sm text-red-400">
              {(updateTool.error as { message?: string })?.message ??
                "Échec de la mise à jour"}
            </span>
          )}
        </div>
      </Section>

      <div className="flex items-center gap-3">
        <Button onClick={() => save.mutate(draft)} disabled={save.isPending}>
          <Save className="h-4 w-4" />
          {save.isPending ? "Enregistrement…" : "Enregistrer"}
        </Button>
        <Button type="button" variant="secondary" onClick={resetDefaults}>
          <RotateCcw className="h-4 w-4" /> Réinitialiser les préférences
        </Button>
        {save.isSuccess && (
          <span className="flex items-center gap-1 text-sm text-emerald-400">
            <Check className="h-4 w-4" /> Enregistré
          </span>
        )}
      </div>
      <p className="text-xs leading-relaxed text-neutral-500">
        « Réinitialiser » remet la qualité, l'audio, les sous-titres, la
        concurrence et la limite de débit aux valeurs par défaut. Le dossier de
        téléchargement et les réglages de l'extension ne sont pas touchés.
      </p>
    </div>
  );
}
