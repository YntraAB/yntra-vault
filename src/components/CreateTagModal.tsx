/**
 * CreateTagModal — Create a new tag (vault category)
 *
 * Name input, color picker (preset palette), validation.
 * Same modal pattern as CreateVaultModal / EntryModal.
 */

import { useState, useEffect, useRef } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { X, Tag as TagIcon, Check } from 'lucide-react';
import { useAppState } from '@/contexts/AppStateContext';

interface CreateTagModalProps {
  open: boolean;
  onClose: () => void;
}

const PRESET_COLORS = [
  '#5b8def', // Blue
  '#5acf7e', // Green
  '#f5a623', // Orange
  '#bd7ee8', // Purple
  '#ef6b6b', // Red
  '#4ecdc4', // Teal
  '#f78fb3', // Pink
  '#778beb', // Indigo
  '#e77f67', // Coral
  '#63cdda', // Cyan
];

export default function CreateTagModal({ open, onClose }: CreateTagModalProps) {
  const { addTag, tags, addToast } = useAppState();
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
      setError('Name is required');
      return;
    }
    if (trimmed.length < 2) {
      setError('Name must be at least 2 characters');
      return;
    }
    if (tags.some((t) => t.name.toLowerCase() === trimmed.toLowerCase())) {
      setError('A tag with this name already exists');
      return;
    }

    addTag({
      id: crypto.randomUUID(),
      name: trimmed,
      color,
      icon: 'tag',
      count: 0,
    });

    addToast({ message: `Tag "${trimmed}" created`, type: 'success' });
    onClose();
  };

  return (
    <AnimatePresence>
      {open && (
        <motion.div
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          className="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
          onClick={onClose}
        >
          <motion.div
            initial={{ scale: 0.96, opacity: 0 }}
            animate={{ scale: 1, opacity: 1 }}
            exit={{ scale: 0.96, opacity: 0 }}
            transition={{ duration: 0.15 }}
            className="w-[380px] rounded-lg border border-[var(--border)] bg-[var(--bg-base)] shadow-2xl"
            onClick={(e) => e.stopPropagation()}
          >
            {/* Header */}
            <div className="flex items-center justify-between border-b border-[var(--border-subtle)] px-5 py-3.5">
              <div className="flex items-center gap-2.5">
                <TagIcon size={16} className="text-[var(--text-primary)]" />
                <h2 className="text-[16px] font-semibold text-[var(--text-primary)]">
                  New Tag
                </h2>
              </div>
              <button
                onClick={onClose}
                className="rounded-md p-1 text-[var(--text-tertiary)] transition-colors hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"
              >
                <X size={16} />
              </button>
            </div>

            {/* Form */}
            <form onSubmit={handleSubmit} className="flex flex-col gap-4 p-5">
              {/* Name */}
              <div className="flex flex-col gap-1.5">
                <label className="text-[12px] font-medium text-[var(--text-secondary)]">
                  Tag Name
                </label>
                <input
                  ref={nameRef}
                  type="text"
                  value={name}
                  onChange={(e) => {
                    setName(e.target.value);
                    setError('');
                  }}
                  placeholder="e.g. Work, Personal, Finance..."
                  className={`h-9 rounded-md border bg-[var(--bg-elevated)] px-3 text-[13px] text-[var(--text-primary)] outline-none placeholder:text-[var(--text-tertiary)] focus:border-[var(--border-focus)] ${
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
                  Color
                </label>
                <div className="flex flex-wrap gap-2">
                  {PRESET_COLORS.map((c) => (
                    <button
                      key={c}
                      type="button"
                      onClick={() => setColor(c)}
                      className="flex h-7 w-7 items-center justify-center rounded-full transition-transform hover:scale-110"
                      style={{
                        backgroundColor: c,
                        boxShadow: color === c ? `0 0 0 2px var(--bg-base), 0 0 0 4px ${c}` : 'none',
                      }}
                    >
                      {color === c && <Check size={14} className="text-white" />}
                    </button>
                  ))}
                </div>
              </div>

              {/* Preview */}
              <div className="flex items-center gap-2 rounded-md bg-[var(--bg-elevated)] px-3 py-2.5">
                <span
                  className="h-2 w-2 rounded-full"
                  style={{ backgroundColor: color }}
                />
                <span className="text-[13px] font-medium text-[var(--text-primary)]">
                  {name.trim() || 'Tag Name'}
                </span>
                <span className="ml-auto text-[11px] tabular-nums text-[var(--text-tertiary)]">
                  0
                </span>
              </div>

              {/* Actions */}
              <div className="flex justify-end gap-2 pt-1">
                <button
                  type="button"
                  onClick={onClose}
                  className="h-9 rounded-md px-4 text-[13px] font-medium text-[var(--text-secondary)] transition-colors hover:bg-[var(--bg-hover)]"
                >
                  Cancel
                </button>
                <button
                  type="submit"
                  className="flex h-9 items-center gap-2 rounded-md bg-[var(--text-primary)] px-4 text-[13px] font-semibold text-[var(--bg-base)] transition-all hover:opacity-90"
                >
                  Create Tag
                </button>
              </div>
            </form>
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}

