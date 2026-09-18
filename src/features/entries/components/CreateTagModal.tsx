/**
 * CreateTagModal — Create a new tag (vault category)
 *
 * Name input, color picker (preset palette), validation.
 * Same modal pattern as CreateVaultModal / EntryModal.
 */

import { useState, useEffect, useRef } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { X, Tag as TagIcon, Check } from 'lucide-react';
import { useEntries } from '../context/EntriesContext';
import { useToast } from '@/contexts/ToastContext';
import { useTranslation } from '@/contexts/LanguageContext';
import { ActionTooltip } from '@/components/ui/tooltip';

export interface CreateTagModalProps {
  open: boolean;
  onClose: () => void;
}

export const PRESET_COLORS = [
  '#5b8def', // Primary blue
  '#4ade80', // Green
  '#f59e0b', // Amber
  '#a855f7', // Purple
  '#ef6b6b', // Red
  '#4ecdc4', // Teal
  '#f78fb3', // Pink
  '#778beb', // Indigo
  '#e77f67', // Coral
  '#63cdda', // Cyan
];

export function CreateTagModal({ open, onClose }: CreateTagModalProps) {
  const { t } = useTranslation();
  const { addTag, tags } = useEntries();
  const { addToast } = useToast();
  const [name, setName] = useState('');
  const [color, setColor] = useState(PRESET_COLORS[0]);
  const [error, setError] = useState('');

  const nameRef = useRef<HTMLInputElement>(null);

  // Focus name field on open
  useEffect(() => {
    if (open) {
      setName('');
      setColor(PRESET_COLORS[Math.floor(Math.random() * PRESET_COLORS.length)]);
      setError('');
      setTimeout(() => nameRef.current?.focus(), 100);
    }
  }, [open]);

  // Esc to close
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && open) onClose();
    };
    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, [open, onClose]);

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();

    const trimmed = name.trim();
    if (!trimmed) {
      setError(t('tags.err_name_req'));
      return;
    }
    if (trimmed.length < 2) {
      setError(t('tags.err_min_length'));
      return;
    }
    if (tags.some((t) => t.name.toLowerCase() === trimmed.toLowerCase())) {
      setError(t('tags.err_exists'));
      return;
    }

    addTag({
      id: crypto.randomUUID(),
      name: trimmed,
      color,
      icon: 'tag',
      count: 0,
    });

    addToast({ message: t('tags.toast_created', { name: trimmed }), type: 'success' });
    onClose();
  };

  return (
    <AnimatePresence>
      {open && (
        <motion.div
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          className="fixed inset-0 z-50 flex items-start sm:items-center justify-center overflow-y-auto bg-black/50 p-3 sm:p-4 touch-pan-y overscroll-contain"
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
                <h2 className="text-[13px] font-semibold text-[var(--text-primary)]">
                  {t('tags.new_tag')}
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
                  0
                </span>
              </div>

              {/* Actions */}
              <div className="flex justify-end gap-2 pt-1 border-t border-[var(--border-subtle)] mt-1">
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
                  {t('sidebar.new_tag')}
                </button>
              </div>
            </form>
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}

export default CreateTagModal;
