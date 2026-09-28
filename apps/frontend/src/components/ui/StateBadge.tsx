import type { TaskStateDto } from "../../types/TaskStateDto";

const styles: Record<TaskStateDto, string> = {
  queued: "bg-neutral-700 text-neutral-200",
  running: "bg-sky-600 text-white",
  paused: "bg-amber-600 text-white",
  completed: "bg-emerald-600 text-white",
  failed: "bg-red-600 text-white",
  cancelled: "bg-neutral-600 text-neutral-200",
};

const labels: Record<TaskStateDto, string> = {
  queued: "En file",
  running: "En cours",
  paused: "En pause",
  completed: "Terminé",
  failed: "Échec",
  cancelled: "Annulé",
};

export function StateBadge({ state }: { state: TaskStateDto }) {
  return (
    <span className={`rounded px-2 py-0.5 text-xs font-medium ${styles[state]}`}>
      {labels[state]}
    </span>
  );
}
