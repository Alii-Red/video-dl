/** Bytes/second → a short human string ("4,2 Mo/s"). */
export function formatSpeed(bps: number | null | undefined): string | null {
  if (bps == null || bps <= 0) return null;
  const units = ["o/s", "Ko/s", "Mo/s", "Go/s"];
  let v = bps;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i++;
  }
  return `${v.toFixed(v >= 100 || i === 0 ? 0 : 1)} ${units[i]}`;
}

/** Seconds remaining → "mm:ss" (or "h:mm:ss" past an hour). */
export function formatEta(secs: number | null | undefined): string | null {
  if (secs == null || secs < 0) return null;
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  const s = Math.floor(secs % 60);
  const mm = String(m).padStart(2, "0");
  const ss = String(s).padStart(2, "0");
  return h > 0 ? `${h}:${mm}:${ss}` : `${mm}:${ss}`;
}
