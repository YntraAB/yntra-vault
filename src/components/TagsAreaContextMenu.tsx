/**
 * TagsAreaContextMenu — Right-click context menu for empty space in sidebar tags section
 */

import { useEffect, useRef } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { Plus, FilterX, ArrowDownAZ, Hash, Check, Trash2, Eye } from 'lucide-react';
import { useTranslation } from '@/contexts/LanguageContext';

interface TagsAreaContextMenuProps {
  open: boolean;
  x: number;
  y: number;
  onClose: () => void;
  onNewTag: () => void;
  onDeselectAll: () => void;
  sortOrder?: 'name' | 'count';
  onSetSortOrder: (order: 'name' | 'count') => void;
  showTagCounts?: boolean;
  onToggleShowTagCounts: () => void;
  onDeleteUnusedTags?: () => void;
  hasUnusedTags?: boolean;
}

export default function TagsAreaContextMenu({
  open,
  x,
  y,
  onClose,
  onNewTag,
  onDeselectAll,
  sortOrder = 'name',
  onSetSortOrder,
  showTagCounts = true,
  onToggleShowTagCounts,
  onDeleteUnusedTags,
  hasUnusedTags = false,
}: TagsAreaContextMenuProps) {
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
    const menuHeight = hasUnusedTags ? 220 : 180;
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
          {/* Actions */}
          <button
            onClick={() => {
              onNewTag();
              onClose();
            }}
            className="flex w-full items-center gap-2.5 px-3 py-1.5 text-left text-[13px] text-[var(--text-secondary)] transition-colors hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"
          >
            <Plus size={13} />
            <span className="flex-1">{t('sidebar.new_tag')}</span>
          </button>

          <button
            onClick={() => {
              onDeselectAll();
              onClose();
            }}
            className="flex w-full items-center gap-2.5 px-3 py-1.5 text-left text-[13px] text-[var(--text-secondary)] transition-colors hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"
          >
            <FilterX size={13} />
            <span className="flex-1">{t('menu.deselect_all')}</span>
          </button>

          <button
            onClick={() => {
              onToggleShowTagCounts();
              onClose();
            }}
            className="flex w-full items-center gap-2.5 px-3 py-1.5 text-left text-[13px] text-[var(--text-secondary)] transition-colors hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"
          >
            <Eye size={13} />
            <span className="flex-1">{t('menu.toggle_tag_counts')}</span>
            {showTagCounts && <Check size={13} className="text-[var(--accent-primary)]" />}
          </button>

          <div className="my-1 h-[1px] bg-[var(--border-subtle)]" />

          {/* Sort options */}
          <button
            onClick={() => {
              onSetSortOrder('name');
              onClose();
            }}
            className="flex w-full items-center gap-2.5 px-3 py-1.5 text-left text-[13px] text-[var(--text-secondary)] transition-colors hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"
          >
            <ArrowDownAZ size={13} />
            <span className="flex-1">{t('menu.sort_name')}</span>
            {sortOrder === 'name' && <Check size={13} className="text-[var(--accent-primary)]" />}
          </button>

          <button
            onClick={() => {
              onSetSortOrder('count');
              onClose();
            }}
            className="flex w-full items-center gap-2.5 px-3 py-1.5 text-left text-[13px] text-[var(--text-secondary)] transition-colors hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"
          >
            <Hash size={13} />
            <span className="flex-1">{t('menu.sort_count')}</span>
            {sortOrder === 'count' && <Check size={13} className="text-[var(--accent-primary)]" />}
          </button>

          {/* Maintenance options */}
          {hasUnusedTags && onDeleteUnusedTags && (
            <>
              <div className="my-1 h-[1px] bg-[var(--border-subtle)]" />
              <button
                onClick={() => {
                  onDeleteUnusedTags();
                  onClose();
                }}
                className="flex w-full items-center gap-2.5 px-3 py-1.5 text-left text-[13px] text-[var(--destructive)] transition-colors hover:bg-[var(--destructive)]/8"
              >
                <Trash2 size={13} />
                <span className="flex-1">{t('menu.delete_unused_tags')}</span>
              </button>
            </>
          )}
        </motion.div>
      )}
    </AnimatePresence>
  );
}
