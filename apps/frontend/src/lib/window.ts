// Live window property mutation for the standard/volatile mode toggle (ADR 1.8).
// Uses the Tauri window API directly — no IPC command needed, but the
// corresponding `core:window:allow-*` permissions must be in the app
// capability (see apps/src-tauri/capabilities/default.json).
import {
  getCurrentWindow,
  currentMonitor,
  LogicalSize,
  LogicalPosition,
} from "@tauri-apps/api/window";
import type { WindowGeometryDto } from "../types/WindowGeometryDto";

const STANDARD = { width: 1100, height: 720 };
const VOLATILE_DEFAULT = { width: 380, height: 480 };

/** A geometry is usable only if it's on-screen-ish and a sane size. A
 * minimized window reports position ~(-32000,-32000) and a tiny size on
 * Windows; persisting/restoring that would place the window off-screen and
 * make the app look like it crashed. */
function isSaneGeometry(g: WindowGeometryDto | null): g is WindowGeometryDto {
  return (
    !!g &&
    g.x > -10000 &&
    g.y > -10000 &&
    g.width >= 200 &&
    g.height >= 150
  );
}

/** Make sure the window is actually shown (not minimized/hidden) before we
 * size or position it. */
async function ensureShown(w: ReturnType<typeof getCurrentWindow>): Promise<void> {
  await w.unminimize().catch(() => {});
  await w.show().catch(() => {});
}

/** Restore the full standard window. */
export async function applyStandard(): Promise<void> {
  const w = getCurrentWindow();
  await ensureShown(w);
  await w.setAlwaysOnTop(false);
  await w.setResizable(true);
  await w.setSize(new LogicalSize(STANDARD.width, STANDARD.height));
  // Guarantee it lands on-screen (it may have been dragged/minimized off it).
  await w.center().catch(() => {});
}

/** Shrink to the compact always-on-top widget, restoring saved geometry. */
export async function applyVolatile(geom: WindowGeometryDto | null): Promise<void> {
  const w = getCurrentWindow();
  await ensureShown(w);
  await w.setAlwaysOnTop(true);
  await w.setResizable(true);

  const usable = isSaneGeometry(geom);
  const width = usable ? geom.width : VOLATILE_DEFAULT.width;
  const height = usable ? geom.height : VOLATILE_DEFAULT.height;
  await w.setSize(new LogicalSize(width, height));

  if (usable) {
    await w.setPosition(new LogicalPosition(geom.x, geom.y));
    return;
  }
  // No usable saved position: park it near the top-right corner of the monitor.
  try {
    const factor = await w.scaleFactor();
    const monitor = await currentMonitor();
    if (monitor) {
      const size = monitor.size.toLogical(factor);
      await w.setPosition(new LogicalPosition(size.width - width - 24, 48));
    }
  } catch {
    // Best-effort placement; if it fails the window just stays where it is.
  }
}

/** Read the current window geometry in logical pixels, for persistence.
 * Returns null when the window is minimized or at a degenerate/off-screen
 * position, so we never persist a geometry that would hide the window on the
 * next launch. */
export async function readGeometry(): Promise<WindowGeometryDto | null> {
  try {
    const w = getCurrentWindow();
    if (await w.isMinimized()) return null;
    const factor = await w.scaleFactor();
    const pos = (await w.outerPosition()).toLogical(factor);
    const size = (await w.innerSize()).toLogical(factor);
    const geom = { x: pos.x, y: pos.y, width: size.width, height: size.height };
    return isSaneGeometry(geom) ? geom : null;
  } catch {
    return null;
  }
}
