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
  browserName,
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
            className={`flex items-start gap-2 py-[3px] px-1.5 rounded-[3px] ${
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
                <CheckCircle2 size={10} className="text-[var(--text-primary)]" />
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
          className="fixed inset-0 z-50 flex items-start sm:items-center justify-center overflow-y-auto bg-black/60 backdrop-blur-[2px] p-3 sm:p-4 touch-pan-y overscroll-contain"
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          transition={{ duration: 0.12 }}
          onClick={onClose}
        >
          <motion.div
            className="relative w-full max-w-[420px] my-auto rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] shadow-2xl overflow-hidden flex flex-col max-h-[calc(100dvh-1.5rem)] sm:max-h-[85vh]"
            initial={{ scale: 0.98, opacity: 0, y: 4 }}
            animate={{ scale: 1, opacity: 1, y: 0 }}
            exit={{ scale: 0.98, opacity: 0, y: 4 }}
            transition={{ duration: 0.15, ease: 'easeOut' }}
            onClick={(e) => e.stopPropagation()}
          >
            {/* Header */}
            <div className="flex items-center justify-between px-4 py-3 border-b border-[var(--border)] bg-[var(--bg-surface)]">
              <div className="flex items-center gap-2.5 min-w-0">
                <div className="flex h-7 w-7 items-center justify-center rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] text-[var(--text-secondary)] shrink-0">
                  <Zap size={13} />
                </div>
                <div className="flex flex-col min-w-0">
                  <span className="text-[13px] font-semibold text-[var(--text-primary)] tracking-tight truncate">
                    Smart Login
                  </span>
                  <span className="text-[11px] text-[var(--text-tertiary)] truncate">
                    {entryTitle}
                  </span>
                </div>
              </div>
              <div className="flex items-center gap-1 shrink-0 ml-2">
                {/* Copy button — shown when there are events */}
                {events.length > 0 && (
                  <button
                    type="button"
                    onClick={handleCopyLog}
                    className="rounded-[3px] p-1 text-[var(--text-tertiary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)] transition-colors"
                    title="Copy log"
                  >
                    {copied ? <Check size={13} className="text-[var(--text-primary)]" /> : <Copy size={13} />}
                  </button>
                )}
                <button
                  type="button"
                  onClick={onClose}
                  className="rounded-[3px] p-1 text-[var(--text-tertiary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)] transition-colors"
                >
                  <X size={14} />
                </button>
              </div>
            </div>

            {/* ───── Phase: Confirm browser close ───── */}
            {phase === 'confirm' && (
              <div className="px-4 py-4 bg-[var(--bg-elevated)]">
                <div className="flex items-start gap-3">
                  <div className="flex h-7 w-7 items-center justify-center rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] text-[var(--text-secondary)] shrink-0 mt-0.5">
                    <MonitorX size={14} />
                  </div>
                  <div className="flex-1 min-w-0">
                    <p className="text-[13px] text-[var(--text-primary)] font-semibold tracking-tight">
                      Close {browserName}?
                    </p>
                    <p className="text-[11px] text-[var(--text-tertiary)] mt-1 leading-relaxed">
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
                    className={`w-3.5 h-3.5 rounded-[3px] border transition-colors flex items-center justify-center shrink-0 ${
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
                <div className="flex justify-end gap-2 mt-4 pt-3 border-t border-[var(--border)]">
                  <button
                    type="button"
                    onClick={onClose}
                    className="h-7 px-3 text-[11px] font-medium rounded-[3px] border border-[var(--border)] bg-[var(--bg-surface)] text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)] transition-colors cursor-pointer"
                  >
                    Cancel
                  </button>
                  <button
                    type="button"
                    onClick={onConfirmClose}
                    className="flex h-7 items-center gap-1 px-3 text-[11px] font-medium rounded-[3px] bg-[var(--text-primary)] text-[var(--bg-base)] hover:opacity-90 transition-opacity cursor-pointer"
                  >
                    Close & continue
                    <ChevronRight size={12} />
                  </button>
                </div>
              </div>
            )}

            {/* ───── Phase: Preparing / Running ───── */}
            {(phase === 'preparing' || phase === 'running') && (
              <div className="px-4 py-3 bg-[var(--bg-elevated)]">
                <div
                  ref={scrollRef}
                  className="max-h-[260px] overflow-y-auto scrollbar-thin"
                >
                  {events.length === 0 && (
                    <div className="flex items-center gap-2 py-5 justify-center">
                      <Loader2 size={13} className="text-[var(--text-tertiary)] animate-spin" />
                      <span className="text-[11px] text-[var(--text-tertiary)]">Preparing...</span>
                    </div>
                  )}
                  {events.length > 0 && renderEventList(events)}
                </div>

                <div className="flex justify-end mt-3 pt-2.5 border-t border-[var(--border)]">
                  <button
                    type="button"
                    onClick={onCancel}
                    className="h-7 px-3 text-[11px] font-medium rounded-[3px] border border-[var(--border)] bg-[var(--bg-surface)] text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)] transition-colors cursor-pointer"
                  >
                    Cancel
                  </button>
                </div>
              </div>
            )}

            {/* ───── Phase: Done ───── */}
            {phase === 'done' && (
              <div className="px-4 py-3 bg-[var(--bg-elevated)]">
                {/* Event log — collapsed, scrollable, selectable */}
                {events.length > 0 && (
                  <div className="max-h-[180px] overflow-y-auto mb-3 scrollbar-thin rounded-[3px] border border-[var(--border)] p-2 bg-[var(--bg-base)]">
                    {renderEventList(events, true)}
                  </div>
                )}

                {/* Result */}
                <div className="flex items-start gap-2.5 px-3 py-2.5 rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)]">
                  {isSuccess ? (
                    <>
                      <CheckCircle2 size={14} className="text-[var(--text-primary)] shrink-0 mt-[1px]" />
                      <span className="text-[12px] text-[var(--text-primary)] font-medium select-text">Login successful</span>
                    </>
                  ) : isCaptcha ? (
                    <>
                      <AlertTriangle size={14} className="text-[var(--text-secondary)] shrink-0 mt-[1px]" />
                      <div>
                        <p className="text-[12px] text-[var(--text-primary)] font-medium">CAPTCHA required</p>
                        <p className="text-[11px] text-[var(--text-tertiary)] mt-0.5 select-text">Complete it in the browser window.</p>
                      </div>
                    </>
                  ) : isMfa ? (
                    <>
                      <AlertTriangle size={14} className="text-[var(--text-secondary)] shrink-0 mt-[1px]" />
                      <div>
                        <p className="text-[12px] text-[var(--text-primary)] font-medium">Two-factor authentication</p>
                        <p className="text-[11px] text-[var(--text-tertiary)] mt-0.5 select-text">Complete 2FA in the browser window.</p>
                      </div>
                    </>
                  ) : isCancelled ? (
                    <>
                      <XCircle size={14} className="text-[var(--text-tertiary)] shrink-0 mt-[1px]" />
                      <span className="text-[12px] text-[var(--text-tertiary)] select-text">Cancelled</span>
                    </>
                  ) : (
                    <>
                      <XCircle size={14} className="text-[var(--text-secondary)] shrink-0 mt-[1px]" />
                      <div className="min-w-0">
                        <p className="text-[12px] text-[var(--text-primary)] font-medium">Login failed</p>
                        {error && (
                          <p className="text-[11px] text-[var(--text-tertiary)] mt-0.5 break-words select-text">{error}</p>
                        )}
                      </div>
                    </>
                  )}
                </div>

                <div className="flex justify-end mt-3 pt-2.5 border-t border-[var(--border)]">
                  <button
                    type="button"
                    onClick={onClose}
                    className="h-7 px-3 text-[11px] font-medium rounded-[3px] bg-[var(--text-primary)] text-[var(--bg-base)] hover:opacity-90 transition-opacity cursor-pointer"
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
