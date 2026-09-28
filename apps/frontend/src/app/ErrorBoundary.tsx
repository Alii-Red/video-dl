import { Component, type ErrorInfo, type ReactNode } from "react";

interface Props {
  children: ReactNode;
}

interface State {
  error: Error | null;
}

/**
 * Catches render errors in the routed page content so a crash (e.g. an
 * infinite update loop, a bad IPC response) shows a recoverable screen
 * instead of blanking the whole app. Must be a class component — React error
 * boundaries have no hook equivalent.
 */
export class ErrorBoundary extends Component<Props, State> {
  state: State = { error: null };

  static getDerivedStateFromError(error: Error): State {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error("Render error caught by ErrorBoundary:", error, info.componentStack);
  }

  private retry = () => this.setState({ error: null });

  render() {
    const { error } = this.state;
    if (!error) return this.props.children;

    return (
      <div className="flex flex-col items-start gap-4 rounded border border-red-900 bg-red-950/40 p-6">
        <div>
          <h2 className="text-lg font-semibold text-red-300">
            Cette page a rencontré une erreur
          </h2>
          <p className="mt-1 max-w-xl text-sm text-neutral-400">{error.message}</p>
        </div>
        <div className="flex gap-2">
          <button
            onClick={this.retry}
            className="rounded bg-neutral-800 px-4 py-2 text-sm font-medium text-white hover:bg-neutral-700"
          >
            Réessayer
          </button>
          <button
            onClick={() => window.location.reload()}
            className="rounded border border-neutral-700 px-4 py-2 text-sm text-neutral-300 hover:bg-neutral-900"
          >
            Recharger l'application
          </button>
        </div>
      </div>
    );
  }
}
