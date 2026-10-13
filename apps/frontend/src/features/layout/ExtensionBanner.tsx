import { useQuery } from "@tanstack/react-query";
import { useNavigate } from "react-router-dom";
import { PlugZap } from "lucide-react";
import { commands } from "../../lib/ipc";

/**
 * Slim status bar shown under the header when live detection is enabled but the
 * browser extension isn't connected — so a new user understands why nothing is
 * being detected, with a one-click path to set it up. Hidden once connected.
 */
export function ExtensionBanner() {
  const navigate = useNavigate();
  const { data: settings } = useQuery({
    queryKey: ["settings"],
    queryFn: () => commands.getSettings(),
  });
  const { data: connected } = useQuery({
    queryKey: ["ext-connected"],
    queryFn: () => commands.extensionConnected(),
    refetchInterval: 3000,
  });

  // Only nag when the bridge is on but no extension has connected yet.
  if (!settings?.extensionEnabled || connected !== false) return null;

  return (
    <button
      onClick={() => navigate("/settings")}
      className="flex w-full items-center gap-2 border-b border-amber-900/50 bg-amber-950/30 px-5 py-1.5 text-left text-xs text-amber-200/90 transition-colors hover:bg-amber-950/50"
    >
      <PlugZap className="h-3.5 w-3.5 shrink-0 text-amber-400" />
      <span>
        Extension non connectée — la détection automatique est inactive pendant
        la navigation.
      </span>
      <span className="ml-auto shrink-0 font-medium text-amber-300 underline-offset-2 hover:underline">
        Configurer
      </span>
    </button>
  );
}
