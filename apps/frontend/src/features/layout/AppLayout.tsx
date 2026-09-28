import { NavLink, Outlet, useLocation } from "react-router-dom";
import { Download, History, Search, Settings } from "lucide-react";
import type { LucideIcon } from "lucide-react";
import { ErrorBoundary } from "../../app/ErrorBoundary";
import { ModeToggle } from "../window/ModeToggle";

const tabs: { to: string; label: string; icon: LucideIcon }[] = [
  { to: "/detect", label: "Détection", icon: Search },
  { to: "/downloads", label: "Téléchargements", icon: Download },
  { to: "/history", label: "Historique", icon: History },
  { to: "/settings", label: "Réglages", icon: Settings },
];

export function AppLayout() {
  const location = useLocation();
  return (
    <div className="flex h-screen flex-col text-neutral-100">
      <header className="flex items-center gap-3 border-b border-neutral-800/80 px-5 py-3 backdrop-blur">
        <div className="flex h-8 w-8 items-center justify-center rounded-lg bg-gradient-to-br from-accent-500 to-accent-700 shadow-sm shadow-accent-600/30">
          <Download className="h-4 w-4 text-white" strokeWidth={2.5} />
        </div>
        <span className="text-base font-semibold tracking-tight">Video DL</span>
        <span className="rounded-md bg-neutral-800/80 px-2 py-0.5 text-[11px] font-medium text-neutral-400">
          local
        </span>
        <div className="ml-auto">
          <ModeToggle />
        </div>
      </header>

      <div className="flex min-h-0 flex-1">
        <nav className="w-56 shrink-0 space-y-1 border-r border-neutral-800/80 p-3">
          {tabs.map(({ to, label, icon: Icon }) => (
            <NavLink
              key={to}
              to={to}
              className={({ isActive }) =>
                `group flex items-center gap-3 rounded-lg px-3 py-2 text-sm transition-colors ${
                  isActive
                    ? "bg-accent-600/15 text-accent-300"
                    : "text-neutral-400 hover:bg-neutral-800/60 hover:text-neutral-200"
                }`
              }
            >
              {({ isActive }) => (
                <>
                  <Icon
                    className={`h-4 w-4 shrink-0 transition-colors ${
                      isActive
                        ? "text-accent-400"
                        : "text-neutral-500 group-hover:text-neutral-300"
                    }`}
                  />
                  {label}
                </>
              )}
            </NavLink>
          ))}
        </nav>

        <main className="min-w-0 flex-1 overflow-y-auto p-6">
          {/* Keyed by path: navigating away and back remounts the boundary,
              clearing any prior error instead of getting stuck on it. */}
          <ErrorBoundary key={location.pathname}>
            <div className="animate-fade-in">
              <Outlet />
            </div>
          </ErrorBoundary>
        </main>
      </div>
    </div>
  );
}
