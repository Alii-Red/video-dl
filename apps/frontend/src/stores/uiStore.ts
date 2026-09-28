import { create } from "zustand";
import type { WindowModeDto } from "../types/WindowModeDto";

interface UiState {
  mode: WindowModeDto;
  /** Boot hasn't finished reading the persisted mode + sizing the window yet. */
  initialized: boolean;
  setMode: (mode: WindowModeDto) => void;
  setInitialized: () => void;
}

/** UI-only: the current window presentation mode. Persisted via Settings. */
export const useUiStore = create<UiState>((set) => ({
  mode: "standard",
  initialized: false,
  setMode: (mode) => set({ mode }),
  setInitialized: () => set({ initialized: true }),
}));
