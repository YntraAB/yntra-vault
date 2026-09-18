/**
 * EditTagModal — Edit or delete an existing tag
 *
 * Pre-filled name/color, delete with confirmation.
 * Same modal pattern as CreateTagModal.
 */

import { useState, useEffect, useRef } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { X, Tag as TagIcon, Check, Trash2 } from 'lucide-react';
import { useEntries } from '../context/EntriesContext';
import { useUi } from '@/contexts/UiContext';
import { useToast } from '@/contexts/ToastContext';
import { useTranslation } from '@/contexts/LanguageContext';
import type { Tag } from '@/types';
import { ActionTooltip } from '@/components/ui/tooltip';
import { DeleteTagModal } from './DeleteTagModal';

export interface EditTagModalProps {
  open: boolean;
  onClose: () => void;
  tag: Tag | null;
}

export const PRESET_COLORS = [
  '#5b8def', '#5acf7e', '#f5a623', '#bd7ee8', '#ef6b6b',
  '#4ecdc4', '#f78fb3', '#778beb', '#e77f67', '#63cdda',
];

export function EditTagModal({ open, onClose, tag }: EditTagModalProps) {
  const { t } = useTranslation();
  const { filterCategory, setFilterCategory } = useUi();
  const { updateTag, removeTag, tags } = useEntries();
  const { addToast } = useToast();
  const [name, setName] = useState('');
  const [color, setColor] = useState(PRESET_COLORS[0]);
  const [error, setError] = useState('');
  const [showDelete, setShowDelete] = useState(false);

  const nameRef = useRef<HTMLInputElement>(null);

  // Populate from tag on open
  useEffect(() => {
    if (open && tag) {
      setName(tag.name);
      setColor(tag.color);
      setError('');
      setShowDelete(false);
      setTimeout(() => nameRef.current?.focus(), 100);
    }
  }, [open, tag]);

  // Esc to close
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && open) {
        if (showDelete) {
          setShowDelete(false);
        } else {
          onClose();
        }
      }
    };
    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, [open, onClose, showDelete]);

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!tag) return;

    const trimmed = name.trim();
    if (!trimmed) {
      setError(t('tags.err_name_req'));
      return;
    }
    if (trimmed.length < 2) {
      setError(t('tags.err_min_length'));
      return;
    }
    if (
      trimmed.toLowerCase() !== tag.name.toLowerCase() &&
      tags.some((t) => t.name.toLowerCase() === trimmed.toLowerCase())
    ) {
      setError(t('tags.err_exists'));
      return;
    }

    updateTag(tag.id, { name: trimmed, color });
    addToast({ message: t('tags.toast_updated'), type: 'success' });
    onClose();
  };

  const handleDelete = () => {
    if (!tag) return;
    if (filterCategory === tag.name) {
      setFilterCategory('all');
    }
    removeTag(tag.id);
    addToast({ message: t('tags.toast_deleted', { name: tag.name }), type: 'info' });
    setShowDelete(false);
    onClose();
  };

  return (
    <>
      <AnimatePresence>
        {open && tag && (
          <motion.div
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-xs select-none"
            onClick={onClose}
          >
            <motion.div
              initial={{ scale: 0.97, opacity: 0 }}
              animate={{ scale: 1, opacity: 1 }}
              exit={{ scale: 0.97, opacity: 0 }}
              transition={{ duration: 0.15 }}
              className="w-full max-w-[380px] mx-3 rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] shadow-2xl overflow-hidden"
              onClick={(e) => e.stopPropagation()}
            >
              {/* Header */}
              <div className="flex items-center justify-between border-b border-[var(--border)] px-4 py-3 bg-[var(--bg-surface)]">
                <div className="flex items-center gap-2.5">
                  <div className="flex h-7 w-7 items-center justify-center rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] text-[var(--text-primary)]">
                    <TagIcon size={14} />
                  </div>
                  <h2 className="text-[13px] font-semibold text-[var(--text-primary)]">
                    {t('menu.edit_tag')}
                  </h2>
                </div>
                <ActionTooltip content={t('common.close')}>
                  <button
                    onClick={onClose}
                    className="rounded-[3px] p-1 text-[var(--text-tertiary)] transition-colors hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)] cursor-pointer"
                  >
                    <X size={15} />
                  </button>
                </ActionTooltip>
              </div>

              {/* Form */}
              <form onSubmit={handleSubmit} className="flex flex-col gap-3.5 p-4">
                {/* Name */}
                <div className="flex flex-col gap-1.5">
                  <label className="text-[12px] font-medium text-[var(--text-secondary)]">
                    {t('tags.tag_name')}
                  </label>
                  <input
                    ref={nameRef}
                    type="text"
                    value={name}
                    onChange={(e) => {
                      setName(e.target.value);
                      setError('');
                    }}
                    placeholder={t('tag.name_ph')}
                    className={`h-8 w-full rounded-[3px] border bg-[var(--bg-base)] px-3 text-[12px] text-[var(--text-primary)] outline-none placeholder:text-[var(--text-tertiary)] focus:border-[var(--border-focus)] transition-colors ${
                      error ? 'border-[var(--destructive)]' : 'border-[var(--border)]'
                    }`}
                  />
                  {error && (
                    <span className="text-[11px] text-[var(--destructive)]">{error}</span>
                  )}
                </div>

                {/* Color */}
                <div className="flex flex-col gap-1.5">
                  <label className="text-[12px] font-medium text-[var(--text-secondary)]">
                    {t('tags.color')}
                  </label>
                  <div className="flex flex-wrap gap-2">
                    {PRESET_COLORS.map((c) => (
                      <button
                        key={c}
                        type="button"
                        onClick={() => setColor(c)}
                        className="flex h-6 w-6 items-center justify-center rounded-[3px] transition-transform hover:scale-105 cursor-pointer"
                        style={{
                          backgroundColor: c,
                          boxShadow: color === c ? '0 0 0 2px var(--bg-base), 0 0 0 3px var(--border-focus)' : 'none',
                        }}
                      >
                        {color === c && <Check size={13} className="text-white" />}
                      </button>
                    ))}
                  </div>
                </div>

                {/* Preview */}
                <div className="flex items-center gap-2 rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] px-3 py-2">
                  <span
                    className="h-2 w-2 rounded-[2px]"
                    style={{ backgroundColor: color }}
                  />
                  <span className="text-[12px] font-medium text-[var(--text-primary)]">
                    {name.trim() || t('tags.tag_name')}
                  </span>
                  <span className="ml-auto text-[11px] tabular-nums text-[var(--text-tertiary)]">
                    {tag.count}
                  </span>
                </div>

                {/* Actions */}
                <div className="flex justify-between border-t border-[var(--border-subtle)] pt-3 mt-1">
                  <button
                    type="button"
                    onClick={() => setShowDelete(true)}
                    className="flex h-8 items-center gap-1.5 rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] px-2.5 text-[12px] font-medium text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:bg-[var(--bg-hover)] transition-colors cursor-pointer"
                  >
                    <Trash2 size={13} />
                    {t('menu.delete')}
                  </button>
                  <div className="flex gap-2">
                    <button
                      type="button"
                      onClick={onClose}
                      className="h-8 rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] px-3 text-[12px] font-medium text-[var(--text-secondary)] transition-colors hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)] cursor-pointer"
                    >
                      {t('common.cancel')}
                    </button>
                    <button
                      type="submit"
                      className="flex h-8 items-center gap-2 rounded-[3px] bg-[var(--text-primary)] px-3.5 text-[12px] font-semibold text-[var(--bg-base)] transition-all hover:opacity-90 cursor-pointer"
                    >
                      {t('common.save_changes')}
                    </button>
                  </div>
                </div>
              </form>
            </motion.div>
          </motion.div>
        )}
      </AnimatePresence>
      <DeleteTagModal
        tag={showDelete ? tag : null}
        onClose={() => setShowDelete(false)}
        onConfirm={handleDelete}
      />
    </>
  );
}

export default EditTagModal;
