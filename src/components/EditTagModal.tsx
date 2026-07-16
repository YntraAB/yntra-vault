/**
 * EditTagModal — Edit or delete an existing tag
 *
 * Pre-filled name/color, delete with confirmation.
 * Same modal pattern as CreateTagModal.
 */

import { useState, useEffect, useRef } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { X, Tag as TagIcon, Check, Trash2, AlertTriangle } from 'lucide-react';
import { useAppState } from '@/contexts/AppStateContext';
import type { Tag } from '@/types';

interface EditTagModalProps {
  open: boolean;
  onClose: () => void;
  tag: Tag | null;
}

const PRESET_COLORS = [
  '#5b8def', '#5acf7e', '#f5a623', '#bd7ee8', '#ef6b6b',
  '#4ecdc4', '#f78fb3', '#778beb', '#e77f67', '#63cdda',
];

export default function EditTagModal({ open, onClose, tag }: EditTagModalProps) {
  const { updateTag, removeTag, tags, addToast } = useAppState();
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
      setError('Name is required');
      return;
    }
    if (trimmed.length < 2) {
      setError('Name must be at least 2 characters');
      return;
    }
    if (
      trimmed.toLowerCase() !== tag.name.toLowerCase() &&
      tags.some((t) => t.name.toLowerCase() === trimmed.toLowerCase())
    ) {
      setError('A tag with this name already exists');
      return;
    }

    updateTag(tag.id, { name: trimmed, color });
    addToast({ message: `Tag updated`, type: 'success' });
    onClose();
  };

  const handleDelete = () => {
    if (!tag) return;
    removeTag(tag.id);
    addToast({ message: `Tag "${tag.name}" deleted`, type: 'info' });
    onClose();
  };

  if (!tag) return null;

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
                  Edit Tag
                </h2>
              </div>
              <button
                onClick={onClose}
                className="rounded-md p-1 text-[var(--text-tertiary)] transition-colors hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"
              >
                <X size={16} />
              </button>
            </div>

            {/* Delete confirmation */}
            <AnimatePresence>
              {showDelete && (
                <motion.div
                  initial={{ height: 0, opacity: 0 }}
                  animate={{ height: 'auto', opacity: 1 }}
                  exit={{ height: 0, opacity: 0 }}
                  transition={{ duration: 0.15 }}
                  className="overflow-hidden border-b border-[var(--border-subtle)]"
                >
                  <div className="flex flex-col gap-3 bg-red-500/5 p-4">
                    <div className="flex items-start gap-2">
                      <AlertTriangle size={16} className="mt-0.5 shrink-0 text-red-400" />
                      <div>
                        <p className="text-[13px] font-medium text-[var(--text-primary)]">
                          Delete "{tag.name}"?
                        </p>
                        <p className="mt-1 text-[12px] text-[var(--text-secondary)]">
                          This will remove the tag from {tag.count} {tag.count === 1 ? 'entry' : 'entries'}. Entries themselves will not be deleted.
                        </p>
                      </div>
                    </div>
                    <div className="flex justify-end gap-2">
                      <button
                        onClick={() => setShowDelete(false)}
                        className="h-8 rounded-md px-3 text-[12px] font-medium text-[var(--text-secondary)] transition-colors hover:bg-[var(--bg-hover)]"
                      >
                        Cancel
                      </button>
                      <button
                        onClick={handleDelete}
                        className="h-8 rounded-md bg-[var(--destructive)] px-3 text-[12px] font-medium text-white transition-colors hover:opacity-90"
                      >
                        Delete Tag
                      </button>
                    </div>
                  </div>
                </motion.div>
              )}
            </AnimatePresence>

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
                  {tag.count}
                </span>
              </div>

              {/* Actions */}
              <div className="flex justify-between border-t border-[var(--border-subtle)] pt-4">
                <button
                  type="button"
                  onClick={() => setShowDelete(true)}
                  className="flex h-9 items-center gap-1.5 rounded-md px-3 text-[13px] font-medium text-[var(--destructive)] transition-colors hover:bg-[var(--destructive)]/8"
                >
                  <Trash2 size={14} />
                  Delete
                </button>
                <div className="flex gap-2">
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
                    Save Changes
                  </button>
                </div>
              </div>
            </form>
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}

