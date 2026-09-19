/**
 * EntryContextMenu — Right-click context menu for entry items in PasswordList
 *
 * Provides quick actions: Rename, Delete, Autotype, Favorite, Pin, Bulk Edit.
 */

import { useEffect, useRef } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { Pencil, Trash2, Zap, Star, Pin, Layers } from 'lucide-react';
import { useTranslation } from '@/contexts/LanguageContext';
import type { PasswordEntry } from '@/types';

export interface EntryContextMenuProps {
  open: boolean;
  x: number;
  y: number;
  entry: PasswordEntry | null;
  selectedCount?: number;
  onClose: () => void;
  onRename: (entry: PasswordEntry) => void;
  onDelete: (entry: PasswordEntry) => void;
  onAutotype: (entry: PasswordEntry) => void;
  onToggleFavorite: (entry: PasswordEntry) => void;
  onTogglePin: (entry: PasswordEntry) => void;
  onBulkEdit?: () => void;
  onBulkDelete?: () => void;
}

export function EntryContextMenu({
  open,
  x,
  y,
  entry,
  selectedCount = 1,
  onClose,
  onRename,
  onDelete,
  onAutotype,
  onToggleFavorite,
  onTogglePin,
  onBulkEdit,
  onBulkDelete,
}: EntryContextMenuProps) {
  const { t } = useTranslation();
  const menuRef = useRef<HTMLDivElement>(null);

  // Close on click/tap outside
  useEffect(() => {
    if (!open) return;
    const handler = (e: Event) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        onClose();
      }
    };
    const timeout = setTimeout(() => {
      document.addEventListener('pointerdown', handler);
      document.addEventListener('mousedown', handler);
    }, 0);
    return () => {
      clearTimeout(timeout);
      document.removeEventListener('pointerdown', handler);
      document.removeEventListener('mousedown', handler);
    };
  }, [open, onClose]);

  // Close on Escape
  useEffect(() => {
    if (!open) return;
    const handler = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose();
    };
    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, [open, onClose]);

  if (!entry) return null;

  const hasUrlOrApp = Boolean(entry.url && entry.url.trim());
  const hasCredentials = Boolean(
    (entry.username && entry.username.trim()) ||
    (entry.email && entry.email.trim()) ||
    (entry.password && entry.password.trim() && entry.password !== '••••••••') ||
    (entry.totpSecret && entry.totpSecret !== 'has-totp') ||
    (entry.customFields && entry.customFields.some(f => f.name !== '_field_order' && f.value && f.value.trim()))
  );
  const canAutotype = hasUrlOrApp && hasCredentials;

  const autotypeTooltip = !hasUrlOrApp
    ? t('context_menu.autotype_no_url_tooltip')
    : !hasCredentials
    ? t('context_menu.autotype_disabled_tooltip')
    : undefined;

  // Adjust position to keep menu in viewport
  const adjustedPosition = () => {
    const menuWidth = 200;
    const menuHeight = selectedCount > 1 ? 160 : 175;
    const adjustedX = Math.min(x, window.innerWidth - menuWidth - 8);
    const adjustedY = Math.min(y, window.innerHeight - menuHeight - 8);
    return { left: adjustedX, top: adjustedY };
  };

  return (
    <AnimatePresence>
      {open && (
        <motion.div
          ref={menuRef}
          initial={{ opacity: 0, scale: 0.95 }}
          animate={{ opacity: 1, scale: 1 }}
          exit={{ opacity: 0, scale: 0.95 }}
          transition={{ duration: 0.1 }}
          className="fixed z-[60] min-w-[200px] rounded-md border border-[var(--border)] bg-[var(--bg-elevated)] py-1 shadow-xl select-none"
          style={adjustedPosition()}
        >
          {selectedCount > 1 && onBulkEdit ? (
            <>
              <button
                onClick={() => {
                  onBulkEdit();
                  onClose();
                }}
                className="flex w-full items-center gap-2.5 px-3 py-1.5 text-left text-[13px] font-medium text-[var(--accent-primary)] transition-colors hover:bg-[var(--bg-hover)]"
              >
                <Layers size={13} />
                <span>Edit {selectedCount} Selected (Ctrl+E)</span>
              </button>

              {onBulkDelete && (
                <button
                  onClick={() => {
                    onBulkDelete();
                    onClose();
                  }}
                  className="flex w-full items-center gap-2.5 px-3 py-1.5 text-left text-[13px] text-[var(--destructive)] transition-colors hover:bg-[var(--destructive)]/8"
                >
                  <Trash2 size={13} />
                  <span>Delete {selectedCount} Selected</span>
                </button>
              )}
            </>
          ) : (
            <>
              <button
                onClick={() => {
                  onRename(entry);
                  onClose();
                }}
                className="flex w-full items-center gap-2.5 px-3 py-1.5 text-left text-[13px] text-[var(--text-secondary)] transition-colors hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"
              >
                <Pencil size={13} />
                {t('menu.edit')}
              </button>

              <button
                disabled={!canAutotype}
                onClick={() => {
                  if (!canAutotype) return;
                  onAutotype(entry);
                  onClose();
                }}
                className={`flex w-full items-center gap-2.5 px-3 py-1.5 text-left text-[13px] transition-colors ${
                  canAutotype
                    ? 'text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)] cursor-pointer'
                    : 'text-[var(--text-tertiary)] opacity-40 cursor-not-allowed'
                }`}
                title={canAutotype ? undefined : autotypeTooltip}
              >
                <Zap size={13} />
                {t('menu.autotype')}
              </button>

              <button
                onClick={() => {
                  onToggleFavorite(entry);
                  onClose();
                }}
                className="flex w-full items-center gap-2.5 px-3 py-1.5 text-left text-[13px] text-[var(--text-secondary)] transition-colors hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"
              >
                <Star size={13} className={entry.favorite ? 'fill-current text-orange-500' : ''} />
                {entry.favorite ? t('menu.unfavorite') : t('menu.favorite')}
              </button>

              <button
                onClick={() => {
                  onTogglePin(entry);
                  onClose();
                }}
                className="flex w-full items-center gap-2.5 px-3 py-1.5 text-left text-[13px] text-[var(--text-secondary)] transition-colors hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"
              >
                <Pin size={13} className={entry.pinned ? 'fill-current text-yellow-500' : ''} />
                {entry.pinned ? t('menu.unpin') : t('menu.pin')}
              </button>

              <div className="my-1 border-t border-[var(--border-subtle)]" />

              <button
                onClick={() => {
                  onDelete(entry);
                  onClose();
                }}
                className="flex w-full items-center gap-2.5 px-3 py-1.5 text-left text-[13px] text-[var(--destructive)] transition-colors hover:bg-[var(--destructive)]/8"
              >
                <Trash2 size={13} />
                {t('menu.delete')}
              </button>
            </>
          )}
        </motion.div>
      )}
    </AnimatePresence>
  );
}

export default EntryContextMenu;
