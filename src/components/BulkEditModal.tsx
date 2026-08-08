/**
 * BulkEditModal — Professional bulk editing modal matching EntryModal card-based layout
 *
 * Bulk editable fields: Title, Username / Email, Tags (Add/Remove), Favorite status, Pin status, Website URL, Notes (Append/Overwrite)
 */

import { useState } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { X, Tag as TagIcon, Star, Pin, Globe, FileText, Check, Plus, Minus, Type, User } from 'lucide-react';
import { useAppState } from '@/contexts/AppStateContext';
import { useTranslation } from '@/contexts/LanguageContext';

interface BulkEditModalProps {
  open: boolean;
  selectedIds: string[];
  onClose: () => void;
}

export default function BulkEditModal({ open, selectedIds, onClose }: BulkEditModalProps) {
  const { t } = useTranslation();
  const { tags: allTags, bulkUpdateEntries } = useAppState();

  // Title & Username / Email
  const [applyTitle, setApplyTitle] = useState(false);
  const [titleValue, setTitleValue] = useState('');

  const [applyUsername, setApplyUsername] = useState(false);
  const [usernameValue, setUsernameValue] = useState('');

  // Tag state mapping: tag.name -> 'keep' | 'add' | 'remove'
  const [tagActions, setTagActions] = useState<Record<string, 'keep' | 'add' | 'remove'>>({});

  // Toggles: 'keep' | 'set-true' | 'set-false'
  const [favAction, setFavAction] = useState<'keep' | 'set-true' | 'set-false'>('keep');
  const [pinAction, setPinAction] = useState<'keep' | 'set-true' | 'set-false'>('keep');

  // URL & Notes
  const [applyUrl, setApplyUrl] = useState(false);
  const [urlValue, setUrlValue] = useState('');

  const [applyNotes, setApplyNotes] = useState(false);
  const [notesValue, setNotesValue] = useState('');
  const [notesMode, setNotesMode] = useState<'append' | 'overwrite'>('append');

  const [isSubmitting, setIsSubmitting] = useState(false);

  const cycleTagAction = (tagName: string) => {
    setTagActions((prev) => {
      const current = prev[tagName] || 'keep';
      if (current === 'keep') return { ...prev, [tagName]: 'add' };
      if (current === 'add') return { ...prev, [tagName]: 'remove' };
      return { ...prev, [tagName]: 'keep' };
    });
  };

  const handleSave = async () => {
    if (selectedIds.length === 0 || isSubmitting) return;

    setIsSubmitting(true);
    try {
      const updates: any = {};

      if (applyTitle) updates.title = titleValue;
      if (applyUsername) updates.username = usernameValue;

      if (favAction === 'set-true') updates.favorite = true;
      if (favAction === 'set-false') updates.favorite = false;

      if (pinAction === 'set-true') updates.pinned = true;
      if (pinAction === 'set-false') updates.pinned = false;

      if (applyUrl) updates.url = urlValue;
      if (applyNotes) updates.notes = notesValue;

      const tagsToAdd = Object.entries(tagActions)
        .filter(([, action]) => action === 'add')
        .map(([name]) => name);

      const tagsToRemove = Object.entries(tagActions)
        .filter(([, action]) => action === 'remove')
        .map(([name]) => name);

      await bulkUpdateEntries(selectedIds, updates, tagsToAdd, tagsToRemove, notesMode);
      onClose();
    } finally {
      setIsSubmitting(false);
    }
  };

  if (!open) return null;

  return (
    <AnimatePresence>
      <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-xs p-4">
        <motion.div
          initial={{ opacity: 0, scale: 0.96 }}
          animate={{ opacity: 1, scale: 1 }}
          exit={{ opacity: 0, scale: 0.96 }}
          transition={{ duration: 0.15 }}
          className="flex max-h-[85vh] w-full max-w-lg flex-col rounded-lg border border-[var(--border)] bg-[var(--bg-elevated)] shadow-2xl overflow-hidden"
        >
          {/* Header */}
          <div className="flex h-13 shrink-0 items-center justify-between border-b border-[var(--border-subtle)] px-5">
            <div className="flex items-center gap-2.5">
              <h2 className="text-[15px] font-semibold text-[var(--text-primary)]">
                Bulk Edit Credentials
              </h2>
              <span className="flex h-5.5 items-center justify-center rounded-full bg-[var(--accent-primary)]/15 px-2.5 text-[11px] font-semibold text-[var(--accent-primary)]">
                {selectedIds.length} entries selected
              </span>
            </div>
            <button
              onClick={onClose}
              className="rounded-[3px] p-1 text-[var(--text-tertiary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)] transition-colors"
            >
              <X size={16} />
            </button>
          </div>

          {/* Form Content */}
          <div className="flex-1 overflow-y-auto p-5 space-y-4">
            {/* Title Card */}
            <div className="flex flex-col gap-2 rounded-lg border border-[var(--border)] bg-[var(--bg-surface)] p-3 shadow-2xs">
              <div className="flex items-center justify-between">
                <label htmlFor="bulk-title-check" className="flex items-center gap-2 text-[12px] font-medium text-[var(--text-secondary)] cursor-pointer">
                  <Type size={14} className="text-[var(--text-tertiary)]" />
                  Title
                </label>
                <div className="flex items-center gap-2">
                  <span className="text-[11px] text-[var(--text-tertiary)]">Update title</span>
                  <input
                    type="checkbox"
                    id="bulk-title-check"
                    checked={applyTitle}
                    onChange={(e) => setApplyTitle(e.target.checked)}
                    className="h-4 w-4 rounded border-[var(--border)] text-[var(--accent-primary)] focus:ring-0 cursor-pointer"
                  />
                </div>
              </div>
              {applyTitle && (
                <input
                  type="text"
                  placeholder={t('bulk.enter_title')}
                  value={titleValue}
                  onChange={(e) => setTitleValue(e.target.value)}
                  className="h-9 w-full rounded-md border border-[var(--border)] bg-[var(--bg-elevated)] px-3 text-[13px] text-[var(--text-primary)] outline-none placeholder:text-[var(--text-tertiary)] focus:border-[var(--border-focus)] transition-colors"
                />
              )}
            </div>

            {/* Username / Email Card */}
            <div className="flex flex-col gap-2 rounded-lg border border-[var(--border)] bg-[var(--bg-surface)] p-3 shadow-2xs">
              <div className="flex items-center justify-between">
                <label htmlFor="bulk-user-check" className="flex items-center gap-2 text-[12px] font-medium text-[var(--text-secondary)] cursor-pointer">
                  <User size={14} className="text-[var(--text-tertiary)]" />
                  Username / Email
                </label>
                <div className="flex items-center gap-2">
                  <span className="text-[11px] text-[var(--text-tertiary)]">Update username</span>
                  <input
                    type="checkbox"
                    id="bulk-user-check"
                    checked={applyUsername}
                    onChange={(e) => setApplyUsername(e.target.checked)}
                    className="h-4 w-4 rounded border-[var(--border)] text-[var(--accent-primary)] focus:ring-0 cursor-pointer"
                  />
                </div>
              </div>
              {applyUsername && (
                <input
                  type="text"
                  placeholder="e.g. john@example.com"
                  value={usernameValue}
                  onChange={(e) => setUsernameValue(e.target.value)}
                  className="h-9 w-full rounded-md border border-[var(--border)] bg-[var(--bg-elevated)] px-3 text-[13px] text-[var(--text-primary)] outline-none placeholder:text-[var(--text-tertiary)] focus:border-[var(--border-focus)] transition-colors"
                />
              )}
            </div>

            {/* Tags Card */}
            <div className="flex flex-col gap-2 rounded-lg border border-[var(--border)] bg-[var(--bg-surface)] p-3 shadow-2xs">
              <label className="flex items-center justify-between text-[12px] font-medium text-[var(--text-secondary)]">
                <span className="flex items-center gap-2">
                  <TagIcon size={14} className="text-[var(--text-tertiary)]" />
                  Tags
                </span>
                <span className="text-[11px] text-[var(--text-tertiary)]">
                  Click chip: Add (+) / Remove (-)
                </span>
              </label>

              {allTags.length === 0 ? (
                <p className="text-[12px] text-[var(--text-tertiary)] italic">No tags in vault.</p>
              ) : (
                <div className="flex flex-wrap gap-1.5 pt-1">
                  {allTags.map((tag) => {
                    const action = tagActions[tag.name] || 'keep';
                    return (
                      <button
                        key={tag.id}
                        type="button"
                        onClick={() => cycleTagAction(tag.name)}
                        className={`flex items-center gap-1.5 rounded-md px-2.5 py-1 text-[12px] font-medium transition-all ${
                          action === 'add'
                            ? 'border border-emerald-500/40 bg-emerald-500/10 text-emerald-500 font-semibold'
                            : action === 'remove'
                            ? 'border border-rose-500/40 bg-rose-500/10 text-rose-500 font-semibold line-through'
                            : 'border border-[var(--border)] bg-[var(--bg-elevated)] text-[var(--text-secondary)] hover:bg-[var(--bg-hover)]'
                        }`}
                      >
                        <span className="h-2 w-2 rounded-full shrink-0" style={{ backgroundColor: tag.color }} />
                        <span>{tag.name}</span>
                        {action === 'add' && <Plus size={12} />}
                        {action === 'remove' && <Minus size={12} />}
                      </button>
                    );
                  })}
                </div>
              )}
            </div>

            {/* Favorites & Pins Card */}
            <div className="flex flex-col gap-2 rounded-lg border border-[var(--border)] bg-[var(--bg-surface)] p-3 shadow-2xs">
              <div className="grid grid-cols-2 gap-4">
                {/* Favorites */}
                <div className="flex flex-col gap-1.5 min-w-0">
                  <label className="flex items-center gap-2 text-[12px] font-medium text-[var(--text-secondary)]">
                    <Star size={14} className="text-[var(--text-tertiary)]" />
                    Favorites
                  </label>
                  <div className="flex rounded-md border border-[var(--border)] bg-[var(--bg-elevated)] p-0.5 gap-0.5">
                    {(['keep', 'set-true', 'set-false'] as const).map((opt) => {
                      const active = favAction === opt;
                      return (
                        <button
                          key={opt}
                          type="button"
                          onClick={() => setFavAction(opt)}
                          className={`flex-1 min-w-0 rounded px-1 py-1 text-[11px] font-medium transition-all text-center truncate ${
                            active
                              ? 'bg-[var(--text-primary)] text-[var(--bg-base)] shadow-xs'
                              : 'text-[var(--text-secondary)] hover:text-[var(--text-primary)]'
                          }`}
                        >
                          {opt === 'keep' ? 'Keep' : opt === 'set-true' ? 'Add' : 'Remove'}
                        </button>
                      );
                    })}
                  </div>
                </div>

                {/* Pin */}
                <div className="flex flex-col gap-1.5 min-w-0">
                  <label className="flex items-center gap-2 text-[12px] font-medium text-[var(--text-secondary)]">
                    <Pin size={14} className="text-[var(--text-tertiary)]" />
                    Pin Status
                  </label>
                  <div className="flex rounded-md border border-[var(--border)] bg-[var(--bg-elevated)] p-0.5 gap-0.5">
                    {(['keep', 'set-true', 'set-false'] as const).map((opt) => {
                      const active = pinAction === opt;
                      return (
                        <button
                          key={opt}
                          type="button"
                          onClick={() => setPinAction(opt)}
                          className={`flex-1 min-w-0 rounded px-1 py-1 text-[11px] font-medium transition-all text-center truncate ${
                            active
                              ? 'bg-[var(--text-primary)] text-[var(--bg-base)] shadow-xs'
                              : 'text-[var(--text-secondary)] hover:text-[var(--text-primary)]'
                          }`}
                        >
                          {opt === 'keep' ? 'Keep' : opt === 'set-true' ? 'Pin' : 'Unpin'}
                        </button>
                      );
                    })}
                  </div>
                </div>
              </div>
            </div>

            {/* Website URL Card */}
            <div className="flex flex-col gap-2 rounded-lg border border-[var(--border)] bg-[var(--bg-surface)] p-3 shadow-2xs">
              <div className="flex items-center justify-between">
                <label htmlFor="bulk-url-check" className="flex items-center gap-2 text-[12px] font-medium text-[var(--text-secondary)] cursor-pointer">
                  <Globe size={14} className="text-[var(--text-tertiary)]" />
                  Website URL
                </label>
                <div className="flex items-center gap-2">
                  <span className="text-[11px] text-[var(--text-tertiary)]">Update URL</span>
                  <input
                    type="checkbox"
                    id="bulk-url-check"
                    checked={applyUrl}
                    onChange={(e) => setApplyUrl(e.target.checked)}
                    className="h-4 w-4 rounded border-[var(--border)] text-[var(--accent-primary)] focus:ring-0 cursor-pointer"
                  />
                </div>
              </div>
              {applyUrl && (
                <input
                  type="text"
                  placeholder="https://example.com"
                  value={urlValue}
                  onChange={(e) => setUrlValue(e.target.value)}
                  className="h-9 w-full rounded-md border border-[var(--border)] bg-[var(--bg-elevated)] px-3 text-[13px] text-[var(--text-primary)] outline-none placeholder:text-[var(--text-tertiary)] focus:border-[var(--border-focus)] transition-colors"
                />
              )}
            </div>

            {/* Notes Card */}
            <div className="flex flex-col gap-2 rounded-lg border border-[var(--border)] bg-[var(--bg-surface)] p-3 shadow-2xs">
              <div className="flex items-center justify-between">
                <label htmlFor="bulk-notes-check" className="flex items-center gap-2 text-[12px] font-medium text-[var(--text-secondary)] cursor-pointer">
                  <FileText size={14} className="text-[var(--text-tertiary)]" />
                  Notes
                </label>
                <div className="flex items-center gap-3">
                  {applyNotes && (
                    <div className="flex items-center gap-2 text-[11px] font-medium text-[var(--text-tertiary)]">
                      <label className="flex items-center gap-1 cursor-pointer hover:text-[var(--text-primary)]">
                        <input
                          type="radio"
                          name="notesMode"
                          checked={notesMode === 'append'}
                          onChange={() => setNotesMode('append')}
                        />
                        Append
                      </label>
                      <label className="flex items-center gap-1 cursor-pointer hover:text-[var(--text-primary)]">
                        <input
                          type="radio"
                          name="notesMode"
                          checked={notesMode === 'overwrite'}
                          onChange={() => setNotesMode('overwrite')}
                        />
                        Overwrite
                      </label>
                    </div>
                  )}
                  <div className="flex items-center gap-1.5">
                    <span className="text-[11px] text-[var(--text-tertiary)]">Update notes</span>
                    <input
                      type="checkbox"
                      id="bulk-notes-check"
                      checked={applyNotes}
                      onChange={(e) => setApplyNotes(e.target.checked)}
                      className="h-4 w-4 rounded border-[var(--border)] text-[var(--accent-primary)] focus:ring-0 cursor-pointer"
                    />
                  </div>
                </div>
              </div>
              {applyNotes && (
                <textarea
                  rows={3}
                  placeholder={t('bulk.notes_placeholder')}
                  value={notesValue}
                  onChange={(e) => setNotesValue(e.target.value)}
                  className="w-full resize-none rounded-md border border-[var(--border)] bg-[var(--bg-elevated)] p-2.5 text-[13px] text-[var(--text-primary)] outline-none placeholder:text-[var(--text-tertiary)] focus:border-[var(--border-focus)] transition-colors"
                />
              )}
            </div>
          </div>

          {/* Footer */}
          <div className="flex h-14 shrink-0 items-center justify-end gap-2 border-t border-[var(--border-subtle)] bg-[var(--bg-surface)] px-5">
            <button
              type="button"
              onClick={onClose}
              className="h-8.5 rounded-[3px] px-3.5 text-[13px] font-medium text-[var(--text-secondary)] transition-colors hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"
            >
              Cancel
            </button>
            <button
              type="button"
              onClick={handleSave}
              disabled={isSubmitting}
              className="flex h-8.5 items-center gap-1.5 rounded-[3px] bg-[var(--text-primary)] px-4 text-[13px] font-medium text-[var(--bg-base)] transition-opacity hover:opacity-90 disabled:opacity-50"
            >
              <Check size={14} />
              {isSubmitting ? 'Updating...' : `Update ${selectedIds.length} Entries`}
            </button>
          </div>
        </motion.div>
      </div>
    </AnimatePresence>
  );
}
