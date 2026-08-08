/**
 * PasswordListAreaContextMenu — Right-click context menu for background area in password list column
 */

import { useEffect, useRef } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { Plus, X, ArrowDownAZ, Clock, Calendar, Check } from 'lucide-react';
import { useTranslation } from '@/contexts/LanguageContext';

interface PasswordListAreaContextMenuProps {
  open: boolean;
  x: number;
  y: number;
  onClose: () => void;
  onNewEntry: () => void;
  searchTerm?: string;
  onClearSearch: () => void;
  sortOrder?: 'title' | 'updated' | 'created';
  onSetSortOrder: (order: 'title' | 'updated' | 'created') => void;
}

export default function PasswordListAreaContextMenu({
  open,
  x,
  y,
  onClose,
  onNewEntry,
  searchTerm = '',
  onClearSearch,
  sortOrder = 'updated',
  onSetSortOrder,
}: PasswordListAreaContextMenuProps) {
  const { t } = useTranslation();
  const menuRef = useRef<HTMLDivElement>(null);

  // Close on click outside
  useEffect(() => {
    if (!open) return;
    const handler = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        onClose();
      }
    };
    const timeout = setTimeout(() => {
      document.addEventListener('mousedown', handler);
    }, 0);
    return () => {
      clearTimeout(timeout);
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

  if (!open) return null;

  const adjustedPosition = () => {
    const menuWidth = 210;
    const menuHeight = searchTerm ? 170 : 135;
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
          className="fixed z-[60] min-w-[210px] rounded-md border border-[var(--border)] bg-[var(--bg-elevated)] py-1 shadow-xl"
          style={adjustedPosition()}
        >
          {/* Quick actions */}
          <button
            onClick={() => {
              onNewEntry();
              onClose();
            }}
            className="flex w-full items-center gap-2.5 px-3 py-1.5 text-left text-[13px] text-[var(--text-secondary)] transition-colors hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"
          >
            <Plus size={13} />
            <span className="flex-1">{t('list.new_entry')}</span>
          </button>

          {searchTerm && (
            <button
              onClick={() => {
                onClearSearch();
                onClose();
              }}
              className="flex w-full items-center gap-2.5 px-3 py-1.5 text-left text-[13px] text-[var(--text-secondary)] transition-colors hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"
            >
              <X size={13} />
              <span className="flex-1">{t('menu.clear_search')}</span>
            </button>
          )}

          <div className="my-1 h-[1px] bg-[var(--border-subtle)]" />

          {/* Sort options */}
          <button
            onClick={() => {
              onSetSortOrder('title');
              onClose();
            }}
            className="flex w-full items-center gap-2.5 px-3 py-1.5 text-left text-[13px] text-[var(--text-secondary)] transition-colors hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"
          >
            <ArrowDownAZ size={13} />
            <span className="flex-1">{t('menu.sort_title')}</span>
            {sortOrder === 'title' && <Check size={13} className="text-[var(--accent-primary)]" />}
          </button>

          <button
            onClick={() => {
              onSetSortOrder('updated');
              onClose();
            }}
            className="flex w-full items-center gap-2.5 px-3 py-1.5 text-left text-[13px] text-[var(--text-secondary)] transition-colors hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"
          >
            <Clock size={13} />
            <span className="flex-1">{t('menu.sort_updated')}</span>
            {sortOrder === 'updated' && <Check size={13} className="text-[var(--accent-primary)]" />}
          </button>

          <button
            onClick={() => {
              onSetSortOrder('created');
              onClose();
            }}
            className="flex w-full items-center gap-2.5 px-3 py-1.5 text-left text-[13px] text-[var(--text-secondary)] transition-colors hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"
          >
            <Calendar size={13} />
            <span className="flex-1">{t('menu.sort_created')}</span>
            {sortOrder === 'created' && <Check size={13} className="text-[var(--accent-primary)]" />}
          </button>
        </motion.div>
      )}
    </AnimatePresence>
  );
}
