import { useEffect } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { commands } from "../../lib/ipc";
import { applyStandard, applyVolatile, readGeometry } from "../../lib/window";
import { useUiStore } from "../../stores/uiStore";
import type { WindowModeDto } from "../../types/WindowModeDto";

/** Flip standard↔volatile: persist the new mode and resize the window live. */
export async function toggleWindowMode(): Promise<void> {
  const current = await commands.getSettings();
  const next: WindowModeDto =
    current.windowMode === "volatile" ? "standard" : "volatile";
  await commands.updateSettings({ ...current, windowMode: next });
  if (next === "volatile") {
    await applyVolatile(current.volatileGeometry ?? null);
  } else {
    await applyStandard();
  }
  useUiStore.getState().setMode(next);
}

/**
 * Boot-time controller: read the persisted mode, size the window accordingly,
 * and — while in volatile mode — persist the widget's geometry as the user
 * moves/resizes it. Mount once, at the app root.
 */
export function useWindowModeController(): void {
  const mode = useUiStore((s) => s.mode);

  // Boot: restore persisted mode + window geometry.
  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const settings = await commands.getSettings();
        if (cancelled) return;
        if (settings.windowMode === "volatile") {
          await applyVolatile(settings.volatileGeometry ?? null);
        } else {
          await applyStandard();
        }
        useUiStore.getState().setMode(settings.windowMode);
      } catch {
        // Not running under Tauri (e.g. browser preview) — stay standard.
      } finally {
        if (!cancelled) useUiStore.getState().setInitialized();
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  // Persist volatile geometry on move/resize (debounced).
  useEffect(() => {
    if (mode !== "volatile") return;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const persist = () => {
      clearTimeout(timer);
      timer = setTimeout(async () => {
        const geom = await readGeometry();
        if (!geom) return;
        try {
          const current = await commands.getSettings();
          await commands.updateSettings({ ...current, volatileGeometry: geom });
        } catch {
          // ignore — geometry persistence is best-effort
        }
      }, 600);
    };

    const unlisteners: Array<() => void> = [];
    const w = getCurrentWindow();
    void w.onMoved(persist).then((u) => unlisteners.push(u));
    void w.onResized(persist).then((u) => unlisteners.push(u));
    return () => {
      clearTimeout(timer);
      unlisteners.forEach((u) => u());
    };
  }, [mode]);
}
