import { Component, type ErrorInfo, type ReactNode } from 'react';
import { AlertTriangle, RefreshCw, Database, Copy, Check } from 'lucide-react';

interface Props {
  children: ReactNode;
}

interface State {
  hasError: boolean;
  error: Error | null;
  errorInfo: ErrorInfo | null;
  copied: boolean;
}

export class ErrorBoundary extends Component<Props, State> {
  constructor(props: Props) {
    super(props);
    this.state = {
      hasError: false,
      error: null,
      errorInfo: null,
      copied: false,
    };
  }

  static getDerivedStateFromError(error: Error): Partial<State> {
    return { hasError: true, error };
  }

  componentDidCatch(error: Error, errorInfo: ErrorInfo) {
    this.setState({ errorInfo });
    console.error('Unhandled React ErrorBoundary caught an error:', error, errorInfo);
  }

  handleReload = () => {
    window.location.reload();
  };

  handleResetToVaultSelect = () => {
    try {
      window.location.hash = '#/';
      window.location.reload();
    } catch {
      window.location.reload();
    }
  };

  handleCopyDetails = () => {
    const { error, errorInfo } = this.state;
    const text = `Error: ${error?.message || 'Unknown error'}\n\nStack:\n${error?.stack || ''}\n\nComponent Stack:\n${errorInfo?.componentStack || ''}`;
    navigator.clipboard.writeText(text).then(() => {
      this.setState({ copied: true });
      setTimeout(() => this.setState({ copied: false }), 2000);
    });
  };

  render() {
    if (this.state.hasError) {
      const { error, copied } = this.state;
      return (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-[var(--bg-base)] p-6 select-none">
          <div className="w-full max-w-md rounded-lg border border-[var(--border)] bg-[var(--bg-elevated)] p-6 shadow-2xl text-center">
            <div className="mx-auto mb-4 flex h-12 w-12 items-center justify-center rounded-full border border-[var(--destructive)]/30 bg-[var(--destructive)]/10 text-[var(--destructive)]">
              <AlertTriangle size={24} />
            </div>

            <h1 className="text-[17px] font-semibold tracking-tight text-[var(--text-primary)]">
              An unexpected error occurred
            </h1>
            <p className="mt-2 text-[13px] leading-relaxed text-[var(--text-secondary)]">
              The application encountered a render issue. Your encrypted database on disk is completely safe.
            </p>

            {error && (
              <div className="mt-4 max-h-32 overflow-y-auto rounded-md border border-[var(--border)] bg-[var(--bg-base)] p-3 text-left font-mono text-[11px] text-[var(--text-tertiary)] select-text">
                <span className="font-semibold text-[var(--destructive)]">{error.name}: </span>
                {error.message}
              </div>
            )}

            <div className="mt-6 flex flex-col gap-2">
              <button
                type="button"
                onClick={this.handleReload}
                className="flex h-9 w-full items-center justify-center gap-2 rounded-md bg-[var(--text-primary)] px-4 text-[13px] font-semibold text-[var(--bg-base)] transition-all hover:opacity-90 cursor-pointer"
              >
                <RefreshCw size={14} />
                <span>Reload Application</span>
              </button>

              <div className="flex gap-2">
                <button
                  type="button"
                  onClick={this.handleResetToVaultSelect}
                  className="flex h-9 flex-1 items-center justify-center gap-2 rounded-md border border-[var(--border)] px-3 text-[12px] font-medium text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)] transition-colors cursor-pointer"
                >
                  <Database size={13} />
                  <span>Vault Select</span>
                </button>

                <button
                  type="button"
                  onClick={this.handleCopyDetails}
                  className="flex h-9 flex-1 items-center justify-center gap-2 rounded-md border border-[var(--border)] px-3 text-[12px] font-medium text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)] transition-colors cursor-pointer"
                >
                  {copied ? <Check size={13} className="text-[var(--success)]" /> : <Copy size={13} />}
                  <span>{copied ? 'Copied' : 'Copy Details'}</span>
                </button>
              </div>
            </div>
          </div>
        </div>
      );
    }

    return this.props.children;
  }
}
