import { useUiStore } from "../../stores/uiStore";
import { toggleWindowMode } from "./useWindowMode";

/** Switches between the full standard window and the compact volatile widget. */
export function ModeToggle({ compact = false }: { compact?: boolean }) {
  const mode = useUiStore((s) => s.mode);
  const label = mode === "volatile" ? "Mode complet" : "Mode widget";

  return (
    <button
      onClick={() => void toggleWindowMode()}
      title={label}
      className={
        compact
          ? "rounded p-1 text-neutral-400 hover:bg-neutral-800 hover:text-white"
          : "rounded border border-neutral-700 px-2 py-1 text-xs text-neutral-300 hover:bg-neutral-800"
      }
    >
      {mode === "volatile" ? "⤢" : "⤡"}
      {!compact && <span className="ml-1">{label}</span>}
    </button>
  );
}
