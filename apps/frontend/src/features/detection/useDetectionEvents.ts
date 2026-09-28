import { useEffect } from "react";
import { events } from "../../lib/ipc";
import { useDetectionsStore } from "../../stores/detectionsStore";

/**
 * Subscribe to `media://detected` and `browsing://context-changed`, feeding
 * both into the global store. Mount once at the app root so detections
 * accumulate regardless of the current window mode or route (manual probe,
 * page extraction, and the Opera GX extension all publish `media://detected`;
 * only the extension publishes context changes).
 */
export function useDetectionEvents(): void {
  const add = useDetectionsStore((s) => s.add);
  const setActiveContext = useDetectionsStore((s) => s.setActiveContext);
  useEffect(() => {
    const unlistenDetected = events.onMediaDetected((p) => add(p.source));
    const unlistenContext = events.onBrowsingContextChanged((p) =>
      setActiveContext(p.tabId, p.pageUrl),
    );
    return () => {
      void unlistenDetected.then((fn) => fn());
      void unlistenContext.then((fn) => fn());
    };
  }, [add, setActiveContext]);
}
