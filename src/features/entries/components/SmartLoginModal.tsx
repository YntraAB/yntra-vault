import { useRef, useEffect, useState } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import {
  Zap, X, Loader2, CheckCircle2, AlertTriangle,
  XCircle, MonitorX, ChevronRight, Copy, Check,
} from 'lucide-react';

// Resolve serde enum state to string
function resolveState(state: unknown): string {
  if (typeof state === 'string') return state;
  if (typeof state === 'object' && state !== null) return Object.keys(state)[0] || '';
  return '';
}

interface SmartLoginEvent {
  timestamp: string;
  state: unknown;
  message: string;
  detail?: unknown;
}

interface SmartLoginModalProps {
  isOpen: boolean;
  onClose: () => void;
  entryTitle: string;
  phase: 'confirm' | 'preparing' | 'running' | 'done';
  events: SmartLoginEvent[];
  result: unknown | null;
  error: string | null;
  browserName: string;
  browserNeedsClose: boolean;
  dontAskAgain: boolean;
  onDontAskAgainChange: (v: boolean) => void;
  onConfirmClose: () => void;
  onCancel: () => void;
}

export default function SmartLoginModal({
  isOpen, onClose, entryTitle, phase,
  events, result, error,
  browserName, browserNeedsClose: _browserNeedsClose,
  dontAskAgain, onDontAskAgainChange,
  onConfirmClose, onCancel,
}: SmartLoginModalProps) {

  const scrollRef = useRef<HTMLDivElement>(null);
  const [copied, setCopied] = useState(false);

  // Auto-scroll event log
  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [events]);

  // Result analysis
  const isSuccess = result && typeof result === 'object' && 'Success' in (result as Record<string, unknown>);
  const isCaptcha = result === 'RequiresCaptcha';
  const isMfa = result && typeof result === 'object' && 'RequiresMfa' in (result as Record<string, unknown>);
  const isCancelled = result === 'Cancelled';
  // eslint-disable-next-line @typescript-eslint/no-unused-vars
  // eslint-disable-next-line @typescript-eslint/no-unused-vars
  // const hasError = error || (phase === 'done' && !isSuccess && !isCaptcha && !isMfa && !isCancelled);

  const handleCopyLog = async () => {
    const text = events.map(e => e.message).join('\n');
    try {
      await navigator.clipboard.writeText(text);
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch { /* clipboard unavailable */ }
  };

  // Event list component shared between running and done phases
  const renderEventList = (items: SmartLoginEvent[], compact = false) => (
    <div className="space-y-px">
      {items.map((event, i) => {
        const isLast = i === items.length - 1 && !compact;
        const state = resolveState(event.state);
        const isTerminal = state === 'Success' || state === 'Cancelled';

        return (
          <motion.div
            key={i}
            className={`flex items-start gap-2 py-[3px] px-1.5 rounded ${
              compact ? '' : 'hover:bg-[var(--bg-hover)]'
            } group`}
            initial={compact ? {} : { opacity: 0, y: 3 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ duration: 0.1 }}
          >
            <div className="mt-[5px] shrink-0 w-3 flex items-center justify-center">
              {isLast && !isTerminal ? (
                <Loader2 size={10} className="text-[var(--text-tertiary)] animate-spin" />
              ) : state === 'Success' ? (
                <CheckCircle2 size={10} className="text-green-500" />
              ) : (
                <div className="w-[4px] h-[4px] rounded-full bg-[var(--text-tertiary)] opacity-30" />
              )}
            </div>
            <span className={`text-[12px] leading-[1.5] select-text ${
              compact
                ? 'text-[var(--text-tertiary)]'
                : isLast
                  ? 'text-[var(--text-secondary)]'
                  : 'text-[var(--text-tertiary)]'
            }`}>
              {event.message}
            </span>
          </motion.div>
        );
      })}
    </div>
  );

  return (
    <AnimatePresence>
      {isOpen && (
        <motion.div
          className="fixed inset-0 z-50 flex items-center justify-center select-none"
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          transition={{ duration: 0.12 }}
        >
          <div className="absolute inset-0 bg-black/50 backdrop-blur-[2px]" onClick={onClose} />

          <motion.div
            className="relative w-full max-w-[400px] mx-4 rounded-lg border border-[var(--border)] bg-[var(--bg-surface)] shadow-xl overflow-hidden"
            initial={{ scale: 0.97, opacity: 0 }}
            animate={{ scale: 1, opacity: 1 }}
            exit={{ scale: 0.97, opacity: 0 }}
            transition={{ duration: 0.15 }}
          >
            {/* Header */}
            <div className="flex items-center justify-between px-4 py-3 border-b border-[var(--border)]">
              <div className="flex items-center gap-2 min-w-0">
                <Zap size={13} className="text-[var(--text-secondary)] shrink-0" />
                <span className="text-[13px] font-medium text-[var(--text-primary)] truncate">
                  Smart Login
                </span>
                <span className="text-[11px] text-[var(--text-tertiary)] truncate">
                  · {entryTitle}
                </span>
              </div>
              <div className="flex items-center gap-1 shrink-0 ml-2">
                {/* Copy button — shown when there are events */}
                {events.length > 0 && (
                  <button
                    onClick={handleCopyLog}
                    className="p-1 rounded text-[var(--text-tertiary)] hover:bg-[var(--bg-hover)] transition-colors"
                    title="Copy log"
                  >
                    {copied ? <Check size={13} className="text-green-500" /> : <Copy size={13} />}
                  </button>
                )}
                <button
                  onClick={onClose}
                  className="p-1 rounded text-[var(--text-tertiary)] hover:bg-[var(--bg-hover)] transition-colors"
                >
                  <X size={14} />
                </button>
              </div>
            </div>

            {/* ───── Phase: Confirm browser close ───── */}
            {phase === 'confirm' && (
              <div className="px-4 py-4">
                <div className="flex items-start gap-3">
                  <div className="mt-0.5 p-1.5 rounded-md bg-[var(--bg-elevated)] border border-[var(--border)]">
                    <MonitorX size={16} className="text-[var(--text-secondary)]" />
                  </div>
                  <div className="flex-1 min-w-0">
                    <p className="text-[13px] text-[var(--text-primary)] font-medium">
                      Close {browserName}?
                    </p>
                    <p className="text-[12px] text-[var(--text-tertiary)] mt-1 leading-relaxed">
                      Smart Login needs to restart {browserName} with debug access to use your profile and cookies. All open tabs will be closed.
                    </p>
                  </div>
                </div>

                {/* Don't ask again */}
                <label className="flex items-center gap-2 mt-4 cursor-pointer select-none group">
                  <button
                    type="button"
                    role="checkbox"
                    aria-checked={dontAskAgain}
                    onClick={() => onDontAskAgainChange(!dontAskAgain)}
                    className={`w-3.5 h-3.5 rounded border transition-colors flex items-center justify-center shrink-0 ${
                      dontAskAgain
                        ? 'bg-[var(--text-secondary)] border-[var(--text-secondary)]'
                        : 'border-[var(--border)] group-hover:border-[var(--text-tertiary)]'
                    }`}
                  >
                    {dontAskAgain && (
                      <svg width="8" height="8" viewBox="0 0 8 8" fill="none">
                        <path d="M1.5 4L3 5.5L6.5 2" stroke="var(--bg-surface)" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round"/>
                      </svg>
                    )}
                  </button>
                  <span
                    className="text-[11px] text-[var(--text-tertiary)]"
                    onClick={() => onDontAskAgainChange(!dontAskAgain)}
                  >
                    Don't ask again
                  </span>
                </label>

                {/* Actions */}
                <div className="flex justify-end gap-2 mt-4">
                  <button
                    onClick={onClose}
                    className="px-3 py-1.5 text-[12px] rounded-md text-[var(--text-tertiary)] hover:bg-[var(--bg-hover)] transition-colors"
                  >
                    Cancel
                  </button>
                  <button
                    onClick={onConfirmClose}
                    className="flex items-center gap-1 px-3 py-1.5 text-[12px] font-medium rounded-md bg-[var(--bg-elevated)] text-[var(--text-primary)] hover:bg-[var(--bg-hover)] border border-[var(--border)] transition-colors"
                  >
                    Close & continue
                    <ChevronRight size={12} />
                  </button>
                </div>
              </div>
            )}

            {/* ───── Phase: Preparing / Running ───── */}
            {(phase === 'preparing' || phase === 'running') && (
              <div className="px-4 py-3">
                <div
                  ref={scrollRef}
                  className="max-h-[260px] overflow-y-auto scrollbar-thin"
                >
                  {events.length === 0 && (
                    <div className="flex items-center gap-2 py-5 justify-center">
                      <Loader2 size={14} className="text-[var(--text-tertiary)] animate-spin" />
                      <span className="text-[12px] text-[var(--text-tertiary)]">Preparing...</span>
                    </div>
                  )}
                  {events.length > 0 && renderEventList(events)}
                </div>

                <div className="flex justify-end mt-3 pt-2 border-t border-[var(--border)]">
                  <button
                    onClick={onCancel}
                    className="px-3 py-1.5 text-[12px] rounded-md text-[var(--text-tertiary)] hover:bg-[var(--bg-hover)] transition-colors"
                  >
                    Cancel
                  </button>
                </div>
              </div>
            )}

            {/* ───── Phase: Done ───── */}
            {phase === 'done' && (
              <div className="px-4 py-3">
                {/* Event log — collapsed, scrollable, selectable */}
                {events.length > 0 && (
                  <div className="max-h-[180px] overflow-y-auto mb-3 scrollbar-thin">
                    {renderEventList(events, true)}
                  </div>
                )}

                {/* Result */}
                <div className={`flex items-start gap-2.5 px-3 py-2.5 rounded-md border ${
                  isSuccess ? 'bg-[var(--bg-elevated)] border-green-500/20' : 'bg-[var(--bg-elevated)] border-[var(--border)]'
                }`}>
                  {isSuccess ? (
                    <>
                      <CheckCircle2 size={15} className="text-green-500 shrink-0 mt-[1px]" />
                      <span className="text-[12px] text-[var(--text-primary)] font-medium select-text">Login successful</span>
                    </>
                  ) : isCaptcha ? (
                    <>
                      <AlertTriangle size={15} className="text-[var(--text-secondary)] shrink-0 mt-[1px]" />
                      <div>
                        <p className="text-[12px] text-[var(--text-primary)] font-medium">CAPTCHA required</p>
                        <p className="text-[11px] text-[var(--text-tertiary)] mt-0.5 select-text">Complete it in the browser window.</p>
                      </div>
                    </>
                  ) : isMfa ? (
                    <>
                      <AlertTriangle size={15} className="text-[var(--text-secondary)] shrink-0 mt-[1px]" />
                      <div>
                        <p className="text-[12px] text-[var(--text-primary)] font-medium">Two-factor authentication</p>
                        <p className="text-[11px] text-[var(--text-tertiary)] mt-0.5 select-text">Complete 2FA in the browser window.</p>
                      </div>
                    </>
                  ) : isCancelled ? (
                    <>
                      <XCircle size={15} className="text-[var(--text-tertiary)] shrink-0 mt-[1px]" />
                      <span className="text-[12px] text-[var(--text-tertiary)] select-text">Cancelled</span>
                    </>
                  ) : (
                    <>
                      <XCircle size={15} className="text-red-400 shrink-0 mt-[1px]" />
                      <div className="min-w-0">
                        <p className="text-[12px] text-[var(--text-primary)] font-medium">Login failed</p>
                        {error && (
                          <p className="text-[11px] text-[var(--text-tertiary)] mt-0.5 break-words select-text">{error}</p>
                        )}
                      </div>
                    </>
                  )}
                </div>

                <div className="flex justify-end mt-3 pt-2 border-t border-[var(--border)]">
                  <button
                    onClick={onClose}
                    className="px-3 py-1.5 text-[12px] font-medium rounded-md bg-[var(--bg-elevated)] text-[var(--text-primary)] hover:bg-[var(--bg-hover)] border border-[var(--border)] transition-colors"
                  >
                    Close
                  </button>
                </div>
              </div>
            )}
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}
