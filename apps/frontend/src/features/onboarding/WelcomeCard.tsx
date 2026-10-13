import { useState } from "react";
import { useNavigate } from "react-router-dom";
import { Download, FolderOpen, Play, Puzzle, Settings, X } from "lucide-react";
import { commands } from "../../lib/ipc";
import { Card } from "../../components/ui/Card";
import { Button } from "../../components/ui/Button";

const KEY = "vdl-onboarded";
const ANIME_SAMA = "https://anime-sama.to";

/** Whether the first-run welcome has been dismissed (per browser profile). */
export function isOnboarded(): boolean {
  try {
    return localStorage.getItem(KEY) === "1";
  } catch {
    return false;
  }
}

function markOnboarded() {
  try {
    localStorage.setItem(KEY, "1");
  } catch {
    // ignore — the card just reappears next launch, harmless
  }
}

/**
 * First-run welcome shown atop the Détection page. Explains the two ways to get
 * started (install the extension, or paste a link) so a brand-new user isn't
 * staring at an empty window. Dismissible, and never shown again once dismissed.
 */
export function WelcomeCard() {
  const navigate = useNavigate();
  const [dismissed, setDismissed] = useState(isOnboarded());
  if (dismissed) return null;

  const dismiss = () => {
    markOnboarded();
    setDismissed(true);
  };

  return (
    <Card className="relative space-y-4 border-accent-800/50 bg-accent-950/20 p-5">
      <button
        onClick={dismiss}
        title="Masquer"
        aria-label="Masquer l'accueil"
        className="absolute right-3 top-3 rounded p-1 text-neutral-500 hover:bg-neutral-800 hover:text-neutral-200"
      >
        <X className="h-4 w-4" />
      </button>

      <div>
        <h3 className="text-lg font-semibold">Bienvenue dans Video DL 👋</h3>
        <p className="mt-1 text-sm text-neutral-400">
          Télécharge tes épisodes d'<span className="text-neutral-200">anime-sama</span> et
          les autres vidéos que tu regardes. Pour commencer :
        </p>
      </div>

      <div className="flex flex-wrap items-center gap-2">
        <Button onClick={() => void commands.openExternal(ANIME_SAMA)}>
          <Play className="h-4 w-4" /> Aller sur anime-sama
        </Button>
        <span className="text-xs text-neutral-500">
          une fois l'extension installée, lance un épisode : il apparaîtra ici.
        </span>
      </div>

      <div className="grid gap-3 sm:grid-cols-2">
        <div className="space-y-2 rounded-lg border border-neutral-800 bg-neutral-900/50 p-3">
          <div className="flex items-center gap-2 text-sm font-medium text-neutral-200">
            <Puzzle className="h-4 w-4 text-accent-400" /> 1. Installer l'extension
          </div>
          <p className="text-xs leading-relaxed text-neutral-400">
            Détecte automatiquement les épisodes (anime-sama, etc.) pendant que
            tu navigues — le token se configure tout seul.
          </p>
          <div className="flex flex-wrap gap-2 pt-1">
            <Button
              size="sm"
              variant="secondary"
              onClick={() => void commands.revealExtensionFolder()}
            >
              <FolderOpen className="h-4 w-4" /> Ouvrir le dossier
            </Button>
            <Button size="sm" variant="ghost" onClick={() => navigate("/settings")}>
              <Settings className="h-4 w-4" /> Réglages
            </Button>
          </div>
        </div>

        <div className="space-y-2 rounded-lg border border-neutral-800 bg-neutral-900/50 p-3">
          <div className="flex items-center gap-2 text-sm font-medium text-neutral-200">
            <Download className="h-4 w-4 text-accent-400" /> 2. Ou coller un lien
          </div>
          <p className="text-xs leading-relaxed text-neutral-400">
            Un lien YouTube ou une page contenant une vidéo : colle-le dans le
            champ ci-dessous et clique <span className="text-neutral-300">Analyser</span>.
          </p>
        </div>
      </div>

      <div className="flex justify-end">
        <Button size="sm" variant="ghost" onClick={dismiss}>
          J'ai compris
        </Button>
      </div>
    </Card>
  );
}
