/**
 * DeleteTagModal — Unified confirmation modal for deleting a tag
 *
 * Used by Sidebar and EditTagModal.
 * Matches DeleteEntryModal visual styling, badge layout, and keyboard accessibility.
 */

import { useEffect, useRef } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { X, Tag as TagIcon } from 'lucide-react';
import { useTranslation } from '@/contexts/LanguageContext';
import type { Tag } from '@/types';
import { ActionTooltip } from '@/components/ui/tooltip';

export interface DeleteTagModalProps {
  tag: Tag | null;
  onClose: () => void;
  onConfirm: () => void;
}

export function DeleteTagModal({ tag, onClose, onConfirm }: DeleteTagModalProps) {
  const { t } = useTranslation();
  const deleteBtnRef = useRef<HTMLButtonElement>(null);
  const cancelBtnRef = useRef<HTMLButtonElement>(null);

  // Focus trap and keyboard navigation (Cancel is focused by default for safety)
  useEffect(() => {
    if (tag) {
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
  }, [tag, onClose]);

  return (
    <AnimatePresence>
      {tag && (
        <div
          className="fixed inset-0 z-[60] flex items-start sm:items-center justify-center overflow-y-auto bg-black/60 backdrop-blur-xs select-none p-3 sm:p-4 touch-pan-y"
          onClick={onClose}
        >
          <motion.div
            initial={{ scale: 0.97, opacity: 0 }}
            animate={{ scale: 1, opacity: 1 }}
            exit={{ scale: 0.97, opacity: 0 }}
            transition={{ duration: 0.15 }}
            className="w-full max-w-[380px] my-auto rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] shadow-2xl overflow-hidden flex flex-col max-h-[calc(100dvh-1.5rem)] sm:max-h-[85vh]"
            onClick={(e) => e.stopPropagation()}
          >
            {/* Header */}
            <div className="flex items-center justify-between border-b border-[var(--border)] px-4 py-3 bg-[var(--bg-surface)]">
              <div className="flex items-center gap-2.5">
                <div className="flex h-7 w-7 items-center justify-center rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] text-[var(--text-primary)]">
                  <TagIcon size={14} />
                </div>
                <h3 className="text-[13px] font-semibold text-[var(--text-primary)]">
                  {t('menu.delete_tag')}
                </h3>
              </div>
              <ActionTooltip content={t('common.close')}>
                <button
                  type="button"
                  onClick={onClose}
                  className="rounded-[3px] p-1 text-[var(--text-tertiary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)] transition-colors cursor-pointer"
                >
                  <X size={15} />
                </button>
              </ActionTooltip>
            </div>

            {/* Content */}
            <div className="p-4">
              <p className="text-[12px] text-[var(--text-secondary)] leading-relaxed">
                {t('delete.confirm_before')}
                <span className="font-semibold text-[var(--text-primary)] bg-[var(--bg-base)] px-2 py-0.5 rounded-[3px] border border-[var(--border)] inline-flex items-center gap-1.5 my-0.5 shadow-2xs align-middle">
                  <span
                    className="h-2 w-2 rounded-full shrink-0"
                    style={{ backgroundColor: tag.color }}
                  />
                  {tag.name}
                </span>
                {t('delete.confirm_after')}
              </p>

              {tag.count > 0 && (
                <p className="mt-2 text-[11px] text-[var(--text-tertiary)] leading-normal">
                  {tag.count === 1
                    ? t('tags.delete_desc_1', { count: tag.count })
                    : t('tags.delete_desc_other', { count: tag.count })}
                </p>
              )}
            </div>

            {/* Footer */}
            <div className="flex justify-end gap-2 border-t border-[var(--border)] px-4 py-3 bg-[var(--bg-surface)]">
              <button
                ref={cancelBtnRef}
                type="button"
                onClick={onClose}
                className="h-8 rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] px-3 text-[12px] font-medium text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)] transition-colors cursor-pointer"
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
                className="h-8 rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] px-3 text-[12px] font-medium text-[var(--text-primary)] hover:bg-[var(--bg-hover)] transition-colors cursor-pointer"
              >
                {t('common.delete')}
              </button>
            </div>
          </motion.div>
        </div>
      )}
    </AnimatePresence>
  );
}

export default DeleteTagModal;
