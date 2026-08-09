/**
 * BulkEditModal — Professional bulk editing modal matching EntryModal card-based layout
 *
 * Bulk editable fields: Title, Username / Email, Tags (Add/Remove), Favorite status, Pin status, Website URL, Notes (Append/Overwrite)
 */

import { useState, useEffect } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { X, Tag as TagIcon, Star, Pin, Globe, FileText, Plus, Minus, Type, User, Loader2 } from 'lucide-react';
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

  // Esc to close
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && open) onClose();
    };
    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, [open, onClose]);

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

  return (
    <AnimatePresence>
      {open && (
        <motion.div
          key="bulk-edit-modal-backdrop"
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
            className="flex max-h-[90vh] w-full max-w-[520px] mx-3 flex-col rounded-lg border border-[var(--border)] bg-[var(--bg-base)] shadow-2xl"
            onClick={(e) => e.stopPropagation()}
          >
            {/* Header */}
            <div className="flex items-center justify-between border-b border-[var(--border-subtle)] px-5 py-3.5">
              <div className="flex items-center gap-2.5">
                <h2 className="text-[16px] font-semibold text-[var(--text-primary)]">
                  Bulk Edit Credentials
                </h2>
                <span className="flex h-5.5 items-center justify-center rounded-full bg-[var(--accent-primary)]/15 px-2.5 text-[11px] font-semibold text-[var(--accent-primary)]">
                  {selectedIds.length} entries selected
                </span>
              </div>
              <button
                onClick={onClose}
                className="rounded-md p-1 text-[var(--text-tertiary)] transition-colors hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"
              >
                <X size={16} />
              </button>
            </div>

            {/* Form Content */}
            <div className="flex flex-col gap-4 overflow-y-auto p-5 flex-1 min-h-0">
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
                    placeholder={t('entry_modal.username_placeholder')}
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
                    {t('sidebar.tags')}
                  </span>
                </label>
                <div className="flex flex-wrap gap-1.5 pt-1">
                  {allTags.map((tag) => {
                    const action = tagActions[tag.name] || 'keep';
                    return (
                      <button
                        key={tag.name}
                        type="button"
                        onClick={() => cycleTagAction(tag.name)}
                        className={`flex items-center gap-1.5 rounded-full px-2.5 py-1 text-[11px] font-medium transition-all cursor-pointer ${
                          action === 'add'
                            ? 'bg-[var(--accent)] text-[var(--bg-base)] shadow-2xs font-semibold'
                            : action === 'remove'
                            ? 'bg-red-500/20 text-red-400 line-through'
                            : 'bg-[var(--bg-elevated)] text-[var(--text-secondary)] hover:bg-[var(--bg-hover)]'
                        }`}
                      >
                        {action === 'add' && <Plus size={12} />}
                        {action === 'remove' && <Minus size={12} />}
                        <span>{tag.name}</span>
                      </button>
                    );
                  })}
                </div>
              </div>

              {/* Favorite & Pin Card */}
              <div className="grid grid-cols-2 gap-3">
                <div className="flex flex-col gap-1.5 rounded-lg border border-[var(--border)] bg-[var(--bg-surface)] p-3 shadow-2xs">
                  <span className="flex items-center gap-1.5 text-[12px] font-medium text-[var(--text-secondary)]">
                    <Star size={14} className="text-[var(--text-tertiary)]" />
                    {t('entry.favorites')}
                  </span>
                  <div className="flex items-center gap-1 pt-1">
                    {(['keep', 'set-true', 'set-false'] as const).map((mode) => (
                      <button
                        key={mode}
                        type="button"
                        onClick={() => setFavAction(mode)}
                        className={`flex-1 rounded-md py-1 text-[11px] font-medium transition-all ${
                          favAction === mode
                            ? 'bg-[var(--accent)] text-[var(--bg-base)] shadow-2xs font-semibold'
                            : 'bg-[var(--bg-elevated)] text-[var(--text-tertiary)] hover:text-[var(--text-secondary)]'
                        }`}
                      >
                        {mode === 'keep' ? t('common.keep') : mode === 'set-true' ? t('common.yes') : t('common.no')}
                      </button>
                    ))}
                  </div>
                </div>

                <div className="flex flex-col gap-1.5 rounded-lg border border-[var(--border)] bg-[var(--bg-surface)] p-3 shadow-2xs">
                  <span className="flex items-center gap-1.5 text-[12px] font-medium text-[var(--text-secondary)]">
                    <Pin size={14} className="text-[var(--text-tertiary)]" />
                    {t('entry.pinned')}
                  </span>
                  <div className="flex items-center gap-1 pt-1">
                    {(['keep', 'set-true', 'set-false'] as const).map((mode) => (
                      <button
                        key={mode}
                        type="button"
                        onClick={() => setPinAction(mode)}
                        className={`flex-1 rounded-md py-1 text-[11px] font-medium transition-all ${
                          pinAction === mode
                            ? 'bg-[var(--accent)] text-[var(--bg-base)] shadow-2xs font-semibold'
                            : 'bg-[var(--bg-elevated)] text-[var(--text-tertiary)] hover:text-[var(--text-secondary)]'
                        }`}
                      >
                        {mode === 'keep' ? t('common.keep') : mode === 'set-true' ? t('common.yes') : t('common.no')}
                      </button>
                    ))}
                  </div>
                </div>
              </div>

              {/* Website URL Card */}
              <div className="flex flex-col gap-2 rounded-lg border border-[var(--border)] bg-[var(--bg-surface)] p-3 shadow-2xs">
                <div className="flex items-center justify-between">
                  <label htmlFor="bulk-url-check" className="flex items-center gap-2 text-[12px] font-medium text-[var(--text-secondary)] cursor-pointer">
                    <Globe size={14} className="text-[var(--text-tertiary)]" />
                    {t('entry.website_url')}
                  </label>
                  <div className="flex items-center gap-1.5">
                    <span className="text-[11px] text-[var(--text-tertiary)]">{t('bulk.update_url')}</span>
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
                    placeholder={t('entry_modal.url_app_placeholder')}
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
                    {t('entry.notes')}
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
                          {t('bulk_edit.append')}
                        </label>
                        <label className="flex items-center gap-1 cursor-pointer hover:text-[var(--text-primary)]">
                          <input
                            type="radio"
                            name="notesMode"
                            checked={notesMode === 'overwrite'}
                            onChange={() => setNotesMode('overwrite')}
                          />
                          {t('bulk_edit.overwrite')}
                        </label>
                      </div>
                    )}
                    <div className="flex items-center gap-1.5">
                      <span className="text-[11px] text-[var(--text-tertiary)]">{t('bulk.update_notes')}</span>
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

              {/* Actions / Footer */}
              <div className="flex justify-end border-t border-[var(--border-subtle)] pt-4 mt-2">
                <div className="flex gap-2">
                  <button
                    type="button"
                    onClick={onClose}
                    className="h-9 rounded-md px-4 text-[13px] font-medium text-[var(--text-secondary)] transition-colors hover:bg-[var(--bg-hover)]"
                  >
                    {t('common.cancel')}
                  </button>
                  <button
                    type="button"
                    onClick={handleSave}
                    disabled={isSubmitting}
                    className="flex h-9 items-center gap-2 rounded-md bg-[var(--text-primary)] px-5 text-[13px] font-semibold text-[var(--bg-base)] transition-all hover:opacity-90 disabled:opacity-50"
                  >
                    {isSubmitting ? (
                      <>
                        <Loader2 size={14} className="animate-spin" />
                        {t('common.loading')}
                      </>
                    ) : (
                      t('bulk_edit.update_count_entries', { count: selectedIds.length })
                    )}
                  </button>
                </div>
              </div>
            </div>
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}

