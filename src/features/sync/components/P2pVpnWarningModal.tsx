import React, { useState, useEffect, useRef } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { ShieldAlert, X, WifiOff, ChevronRight } from 'lucide-react';
import { useTranslation } from '@/contexts/LanguageContext';

export const P2P_VPN_STORAGE_KEY = 'yntra-vault-p2p-vpn-warning-dismissed';

export interface P2pVpnWarningModalProps {
  isOpen: boolean;
  onClose: () => void;
  onConfirm: () => void;
}

export const P2pVpnWarningModal: React.FC<P2pVpnWarningModalProps> = ({
  isOpen,
  onClose,
  onConfirm,
}) => {
  const { t } = useTranslation();
  const continueBtnRef = useRef<HTMLButtonElement>(null);

  const [dontAskAgain, setDontAskAgain] = useState<boolean>(() => {
    try {
      return localStorage.getItem(P2P_VPN_STORAGE_KEY) === 'true';
    } catch {
      return false;
    }
  });

  // Focus Continue button on open & handle Escape
  useEffect(() => {
    if (isOpen) {
      const timer = setTimeout(() => continueBtnRef.current?.focus(), 50);

      const handleKeyDown = (e: KeyboardEvent) => {
        if (e.key === 'Escape') {
          e.preventDefault();
          e.stopPropagation();
          onClose();
        }
      };

      window.addEventListener('keydown', handleKeyDown, true);
      return () => {
        clearTimeout(timer);
        window.removeEventListener('keydown', handleKeyDown, true);
      };
    }
  }, [isOpen, onClose]);

  const handleConfirm = () => {
    try {
      if (dontAskAgain) {
        localStorage.setItem(P2P_VPN_STORAGE_KEY, 'true');
      } else {
        localStorage.removeItem(P2P_VPN_STORAGE_KEY);
      }
    } catch (err) {
      console.warn('Could not persist VPN preference:', err);
    }
    onConfirm();
  };

  return (
    <AnimatePresence>
      {isOpen && (
        <motion.div
          className="fixed inset-0 z-[70] flex items-start sm:items-center justify-center overflow-y-auto bg-black/60 backdrop-blur-[2px] p-3 sm:p-4 touch-pan-y overscroll-contain"
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          transition={{ duration: 0.12 }}
          onClick={onClose}
        >
          <motion.div
            initial={{ scale: 0.98, opacity: 0, y: 4 }}
            animate={{ scale: 1, opacity: 1, y: 0 }}
            exit={{ scale: 0.98, opacity: 0, y: 4 }}
            transition={{ duration: 0.15, ease: 'easeOut' }}
            className="relative w-full max-w-[420px] my-auto rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] shadow-2xl overflow-hidden flex flex-col max-h-[calc(100dvh-1.5rem)] sm:max-h-[85vh]"
            onClick={(e) => e.stopPropagation()}
          >
            {/* Header — Identical to SmartLoginModal */}
            <div className="flex items-center justify-between px-4 py-3 border-b border-[var(--border)] bg-[var(--bg-surface)]">
              <div className="flex items-center gap-2.5 min-w-0">
                <div className="flex h-7 w-7 items-center justify-center rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] text-[var(--text-secondary)] shrink-0">
                  <ShieldAlert size={14} />
                </div>
                <div className="flex flex-col min-w-0">
                  <span className="text-[13px] font-semibold text-[var(--text-primary)] tracking-tight truncate">
                    {t('pairing.vpn_warning_title') || 'VPN & Nätverk'}
                  </span>
                  <span className="text-[11px] text-[var(--text-tertiary)] truncate">
                    {t('pairing.vpn_warning_subtitle') || 'Lokal nätverksåtkomst krävs'}
                  </span>
                </div>
              </div>
              <button
                type="button"
                onClick={onClose}
                className="rounded-[3px] p-1 text-[var(--text-tertiary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)] transition-colors cursor-pointer"
              >
                <X size={14} />
              </button>
            </div>

            {/* Body — Identical to SmartLoginModal phase confirm */}
            <div className="px-4 py-4 bg-[var(--bg-elevated)]">
              <div className="flex items-start gap-3">
                <div className="flex h-7 w-7 items-center justify-center rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] text-[var(--text-secondary)] shrink-0 mt-0.5">
                  <WifiOff size={14} />
                </div>
                <div className="flex-1 min-w-0">
                  <p className="text-[13px] text-[var(--text-primary)] font-semibold tracking-tight">
                    {t('pairing.vpn_warning_heading') || 'Aktiv VPN kan blockera enheter'}
                  </p>
                  <p className="text-[11px] text-[var(--text-tertiary)] mt-1 leading-relaxed">
                    {t('pairing.vpn_warning_desc') ||
                      'Aktiv VPN blockerar lokal anslutning mellan enheterna på nätverket. Koppla från VPN under överföringen.'}
                  </p>
                </div>
              </div>

              {/* Don't ask again checkbox — Styled exactly like Smart Login */}
              <label className="flex items-center gap-2 mt-4 cursor-pointer select-none group">
                <button
                  type="button"
                  role="checkbox"
                  aria-checked={dontAskAgain}
                  onClick={() => setDontAskAgain(!dontAskAgain)}
                  className={`w-3.5 h-3.5 rounded-[3px] border transition-colors flex items-center justify-center shrink-0 ${
                    dontAskAgain
                      ? 'bg-[var(--text-secondary)] border-[var(--text-secondary)]'
                      : 'border-[var(--border)] group-hover:border-[var(--text-tertiary)]'
                  }`}
                >
                  {dontAskAgain && (
                    <svg width="8" height="8" viewBox="0 0 8 8" fill="none">
                      <path
                        d="M1.5 4L3 5.5L6.5 2"
                        stroke="var(--bg-surface)"
                        strokeWidth="1.5"
                        strokeLinecap="round"
                        strokeLinejoin="round"
                      />
                    </svg>
                  )}
                </button>
                <span
                  className="text-[11px] text-[var(--text-tertiary)]"
                  onClick={() => setDontAskAgain(!dontAskAgain)}
                >
                  {t('pairing.vpn_dont_show_again') || 'Visa inte denna varning igen'}
                </span>
              </label>

              {/* Actions — Cancel & Continue (clean and standard) */}
              <div className="flex justify-end gap-2 mt-4 pt-3 border-t border-[var(--border)]">
                <button
                  type="button"
                  onClick={onClose}
                  className="h-7 px-3 text-[11px] font-medium rounded-[3px] border border-[var(--border)] bg-[var(--bg-surface)] text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)] transition-colors cursor-pointer"
                >
                  {t('common.cancel')}
                </button>
                <button
                  ref={continueBtnRef}
                  type="button"
                  onClick={handleConfirm}
                  className="flex h-7 items-center gap-1 px-3 text-[11px] font-medium rounded-[3px] bg-[var(--text-primary)] text-[var(--bg-base)] hover:opacity-90 transition-opacity cursor-pointer"
                >
                  {t('common.continue')}
                  <ChevronRight size={12} />
                </button>
              </div>
            </div>
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>
  );
};

export default P2pVpnWarningModal;
