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

  const isSingle = mode === 'single';
  const title = isSingle
    ? (t('settings.delete_permanently_title') || t('delete.title') || 'Permanently Delete Entry')
    : (t('settings.empty_trash_title') || t('settings.trash_empty') || 'Empty Trash');

  return (
    <AnimatePresence>
      {isOpen && (
        <motion.div
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          className="fixed inset-0 z-[60] flex items-start sm:items-center justify-center overflow-y-auto bg-black/60 p-3 sm:p-4 touch-pan-y overscroll-contain"
          onClick={onClose}
        >
          <motion.div
            initial={{ scale: 0.97, opacity: 0, y: 6 }}
            animate={{ scale: 1, opacity: 1, y: 0 }}
            exit={{ scale: 0.97, opacity: 0, y: 6 }}
            transition={{ duration: 0.15, ease: 'easeOut' }}
            className="w-full max-w-[400px] my-auto rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] shadow-xl overflow-hidden flex flex-col max-h-[calc(100dvh-1.5rem)] sm:max-h-[85vh]"
            onClick={(e) => e.stopPropagation()}
          >
            {/* Header */}
            <div className="flex items-center justify-between border-b border-[var(--border-subtle)] px-5 py-3.5 bg-[var(--bg-base)] shrink-0">
              <div className="flex items-center gap-2.5">
                <div className="flex h-7 w-7 items-center justify-center rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] text-[var(--text-secondary)]">
                  <AlertTriangle size={14} />
                </div>
                <div>
                  <h2 className="text-[14px] font-medium text-[var(--text-primary)] leading-tight">
                    {title}
                  </h2>
                  <p className="text-[11px] text-[var(--text-tertiary)]">
                    {isSingle ? t('settings.trash_title') : t('settings.storage_trash')}
                  </p>
                </div>
              </div>
              <ActionTooltip content={t('common.close')}>
                <button
                  type="button"
                  onClick={onClose}
                  className="rounded-[3px] p-1 text-[var(--text-tertiary)] transition-colors hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)] cursor-pointer"
                >
                  <X size={15} />
                </button>
              </ActionTooltip>
            </div>

            {/* Body */}
            <div className="p-5 flex flex-col gap-3 flex-1 min-h-0 overflow-y-auto touch-pan-y overscroll-contain">
              {isSingle ? (
                <p className="text-[12px] text-[var(--text-secondary)] leading-relaxed">
                  {t('delete.confirm_before')}{' '}
                  <span className="font-semibold text-[var(--text-primary)] bg-[var(--bg-base)] px-1.5 py-0.5 rounded-[3px] border border-[var(--border)] inline-block select-text">
                    {itemTitle || ''}
                  </span>{' '}
                  {t('delete.confirm_after')}
                </p>
              ) : (
                <div className="flex flex-col gap-2.5">
                  <p className="text-[12px] text-[var(--text-secondary)] leading-relaxed">
                    {t('settings.confirm_empty_trash') || 'Are you sure you want to permanently delete all items in the trash? This action cannot be undone.'}
                  </p>
                  {itemCount !== undefined && itemCount > 0 && (
                    <div className="flex items-center gap-2 rounded-[3px] border border-[var(--border-subtle)] bg-[var(--bg-base)] px-2.5 py-1.5 text-[11px] font-medium text-[var(--text-secondary)]">
                      <span>
                        {itemCount === 1
                          ? (t('settings.trash_items_warning_singular', { count: 1 }) || '1 item will be permanently erased.')
                          : (t('settings.trash_items_warning', { count: itemCount }) || `${itemCount} items will be permanently erased.`)}
                      </span>
                    </div>
                  )}
                </div>
              )}
            </div>

            {/* Footer */}
            <div className="flex justify-end gap-2 border-t border-[var(--border-subtle)] px-5 py-3 bg-[var(--bg-base)]">
              <button
                ref={cancelBtnRef}
                type="button"
                onClick={onClose}
                className="h-8 rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] px-3 text-[12px] font-medium text-[var(--text-secondary)] transition-colors hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)] cursor-pointer"
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
                className="h-8 rounded-[3px] border border-[var(--border)] bg-[var(--text-primary)] px-3.5 text-[12px] font-semibold text-[var(--bg-base)] hover:opacity-90 transition-opacity cursor-pointer"
              >
                {isSingle ? (t('settings.delete_permanently') || t('common.delete')) : (t('settings.trash_empty') || t('common.delete'))}
              </button>
            </div>
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}

export default DeleteTrashModal;
