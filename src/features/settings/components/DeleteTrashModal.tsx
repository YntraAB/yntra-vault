/**
 * DeleteTrashModal — Confirmation modal for permanently deleting trash items or emptying trash
 *
 * Matches DeleteEntryModal and DeleteTagModal visual styling, badge layout, and keyboard accessibility.
 * Renders with z-[60] to properly overlay the settings drawer.
 */

import { useEffect, useRef } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { X, AlertTriangle } from 'lucide-react';
import { useTranslation } from '@/contexts/LanguageContext';
import { ActionTooltip } from '@/components/ui/tooltip';

export interface DeleteTrashModalProps {
  isOpen: boolean;
  mode: 'single' | 'empty';
  itemTitle?: string;
  itemCount?: number;
  onClose: () => void;
  onConfirm: () => void;
}

export function DeleteTrashModal({
  isOpen,
  mode,
  itemTitle,
  itemCount,
  onClose,
  onConfirm,
}: DeleteTrashModalProps) {
  const { t } = useTranslation();
  const deleteBtnRef = useRef<HTMLButtonElement>(null);
  const cancelBtnRef = useRef<HTMLButtonElement>(null);

  // Focus trap and keyboard navigation (Cancel is focused by default for safety)
  useEffect(() => {
    if (isOpen) {
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
  }, [isOpen, onClose]);

  if (!isOpen) return null;

  const isSingle = mode === 'single';
  const title = isSingle
    ? (t('settings.delete_permanently_title') || t('delete.title') || 'Permanently Delete Entry')
    : (t('settings.empty_trash_title') || t('settings.trash_empty') || 'Empty Trash');

  return (
    <AnimatePresence>
      <div
        className="fixed inset-0 z-[60] flex items-center justify-center bg-black/50 select-none"
        onClick={onClose}
      >
        <motion.div
          initial={{ scale: 0.98, opacity: 0 }}
          animate={{ scale: 1, opacity: 1 }}
          exit={{ scale: 0.98, opacity: 0 }}
          transition={{ duration: 0.15 }}
          className="w-full max-w-[380px] mx-3 rounded-lg border border-[var(--border)] bg-[var(--bg-base)] p-5 shadow-2xl"
          onClick={(e) => e.stopPropagation()}
        >
          <div className="flex items-center justify-between pb-3 border-b border-[var(--border-subtle)]">
            <h3 className="text-[16px] font-semibold text-[var(--text-primary)]">
              {title}
            </h3>
            <ActionTooltip content={t('common.close')}>
              <button
                type="button"
                onClick={onClose}
                className="rounded p-1 text-[var(--text-tertiary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)] cursor-pointer"
              >
                <X size={16} />
              </button>
            </ActionTooltip>
          </div>

          {isSingle ? (
            <p className="mt-4 text-[13px] text-[var(--text-secondary)] leading-relaxed">
              {t('delete.confirm_before')}
              <span className="font-bold text-[var(--text-primary)] bg-[var(--bg-elevated)] px-2 py-0.5 rounded border border-[var(--border)] inline-block my-0.5 shadow-sm select-text">
                {itemTitle || ''}
              </span>
              {t('delete.confirm_after')}
            </p>
          ) : (
            <div className="mt-4 flex flex-col gap-2.5">
              <p className="text-[13px] text-[var(--text-secondary)] leading-relaxed">
                {t('settings.confirm_empty_trash') || 'Are you sure you want to permanently delete all items in the trash? This action cannot be undone.'}
              </p>
              {itemCount !== undefined && itemCount > 0 && (
                <div className="flex items-center gap-2 rounded-[3px] border border-red-500/30 bg-red-500/10 px-2.5 py-1.5 text-[11px] font-medium text-red-500">
                  <AlertTriangle size={13} className="shrink-0" />
                  <span>{itemCount} {itemCount === 1 ? 'item' : 'items'} will be permanently erased.</span>
                </div>
              )}
            </div>
          )}

          <div className="mt-5 flex justify-end gap-2 pt-3 border-t border-[var(--border-subtle)]">
            <button
              ref={cancelBtnRef}
              type="button"
              onClick={onClose}
              className="h-9 rounded-md border border-[var(--border)] px-4 text-[13px] font-medium text-[var(--text-secondary)] transition-colors hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)] focus:outline-none focus:ring-2 focus:ring-[var(--border-focus)] cursor-pointer"
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
              className="h-9 rounded-md bg-red-600 px-4 text-[13px] font-semibold text-white transition-all hover:bg-red-700 active:bg-red-800 shadow-sm focus:outline-none focus:ring-2 focus:ring-red-500/50 cursor-pointer"
            >
              {isSingle ? (t('settings.delete_permanently') || t('common.delete')) : (t('settings.trash_empty') || t('common.delete'))}
            </button>
          </div>
        </motion.div>
      </div>
    </AnimatePresence>
  );
}

export default DeleteTrashModal;
