/**
 * DeleteEntryModal — Unified confirmation modal for deleting an entry
 *
 * Used by PasswordList and PasswordDetail.
 * Features red action button, highlighted entry name badge, and focus trapping.
 */

import { useEffect, useRef } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { X } from 'lucide-react';
import { useTranslation } from '@/contexts/LanguageContext';
import type { PasswordEntry } from '@/types';
import { ActionTooltip } from './ui/tooltip';

interface DeleteEntryModalProps {
  entry: PasswordEntry | null;
  onClose: () => void;
  onConfirm: () => void;
}

export default function DeleteEntryModal({ entry, onClose, onConfirm }: DeleteEntryModalProps) {
  const { t } = useTranslation();
  const deleteBtnRef = useRef<HTMLButtonElement>(null);
  const cancelBtnRef = useRef<HTMLButtonElement>(null);

  // Focus trap and keyboard navigation (Cancel is focused by default for safety)
  useEffect(() => {
    if (entry) {
      const timer = setTimeout(() => cancelBtnRef.current?.focus(), 50);

      const handleKeyDown = (e: KeyboardEvent) => {
        if (e.key === 'Escape') {
          e.preventDefault();
          e.stopPropagation();
          onClose();
        } else if (e.key === 'Tab') {
          e.preventDefault();
          if (document.activeElement === cancelBtnRef.current) {
            deleteBtnRef.current?.focus();
          } else {
            cancelBtnRef.current?.focus();
          }
        }
      };

      window.addEventListener('keydown', handleKeyDown, true);
      return () => {
        clearTimeout(timer);
        window.removeEventListener('keydown', handleKeyDown, true);
      };
    }
  }, [entry, onClose]);

  if (!entry) return null;

  return (
    <AnimatePresence>
      <div
        className="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
        onClick={onClose}
      >
        <motion.div
          initial={{ scale: 0.98, opacity: 0 }}
          animate={{ scale: 1, opacity: 1 }}
          exit={{ scale: 0.98, opacity: 0 }}
          transition={{ duration: 0.15 }}
          className="w-[380px] rounded-lg border border-[var(--border)] bg-[var(--bg-base)] p-5 shadow-2xl"
          onClick={(e) => e.stopPropagation()}
        >
          <div className="flex items-center justify-between pb-3 border-b border-[var(--border-subtle)]">
            <h3 className="text-[16px] font-semibold text-[var(--text-primary)]">
              {t('delete.title')}
            </h3>
            <ActionTooltip content={t('common.close')}>
              <button
                type="button"
                onClick={onClose}
                className="rounded p-1 text-[var(--text-tertiary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"
              >
                <X size={16} />
              </button>
            </ActionTooltip>
          </div>

          <p className="mt-4 text-[13px] text-[var(--text-secondary)] leading-relaxed">
            {t('delete.confirm_before')}<span className="font-bold text-[var(--text-primary)] bg-[var(--bg-elevated)] px-2 py-0.5 rounded border border-[var(--border)] inline-block my-0.5 shadow-sm">{entry.title}</span>{t('delete.confirm_after')}
          </p>

          <div className="mt-5 flex justify-end gap-2 pt-3 border-t border-[var(--border-subtle)]">
            <button
              ref={cancelBtnRef}
              type="button"
              onClick={onClose}
              className="h-9 rounded-md border border-[var(--border)] px-4 text-[13px] font-medium text-[var(--text-secondary)] transition-colors hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)] focus:outline-none focus:ring-2 focus:ring-[var(--border-focus)]"
            >
              {t('common.cancel')}
            </button>
            <button
              ref={deleteBtnRef}
              type="button"
              onClick={() => {
                onConfirm();
                onClose();
              }}
              className="h-9 rounded-md bg-red-600 px-4 text-[13px] font-semibold text-white transition-all hover:bg-red-700 active:bg-red-800 shadow-sm focus:outline-none focus:ring-2 focus:ring-red-500/50"
            >
              {t('common.delete')}
            </button>
          </div>
        </motion.div>
      </div>
    </AnimatePresence>
  );
}
