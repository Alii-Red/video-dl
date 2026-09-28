export function ProgressBar({ percent }: { percent: number }) {
  const clamped = Math.max(0, Math.min(100, percent));
  // Before yt-dlp reports real progress (still 0), show an indeterminate
  // shimmer so the bar doesn't look stuck/empty.
  const indeterminate = clamped === 0;
  return (
    <div className="h-2 w-full overflow-hidden rounded-full bg-neutral-800">
      {indeterminate ? (
        <div
          className="h-full w-1/3 rounded-full"
          style={{
            background:
              "linear-gradient(90deg, transparent, var(--color-accent-500), transparent)",
            backgroundSize: "200% 100%",
            animation: "shimmer 1.2s ease-in-out infinite",
          }}
        />
      ) : (
        <div
          className="h-full rounded-full bg-gradient-to-r from-accent-500 to-accent-400 transition-[width] duration-300 ease-out"
          style={{ width: `${clamped}%` }}
        />
      )}
    </div>
  );
}
