import { RouterProvider } from "react-router-dom";
import { router } from "./router";
import { useUiStore } from "../stores/uiStore";
import { useWindowModeController } from "../features/window/useWindowMode";
import { useDetectionEvents } from "../features/detection/useDetectionEvents";
import { VolatileLayout } from "../features/layout/VolatileLayout";

/**
 * App root: owns the window-mode lifecycle and the global detection
 * subscription, then renders either the full routed app (standard) or the
 * compact widget (volatile). A single window, two presentations (ADR 1.8).
 */
export function Root() {
  useWindowModeController();
  useDetectionEvents();

  const mode = useUiStore((s) => s.mode);
  const initialized = useUiStore((s) => s.initialized);

  // Avoid a flash of the standard layout before the persisted mode is applied.
  if (!initialized) {
    return <div className="h-screen bg-neutral-950" />;
  }

  return mode === "volatile" ? <VolatileLayout /> : <RouterProvider router={router} />;
}
