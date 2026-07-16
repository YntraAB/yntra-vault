/**
 * TagContextMenu — Right-click context menu for sidebar tags
 *
 * Shows Edit / Delete options. Positioned at cursor.
 * Closes on click outside or Escape.
 */

import { useEffect, useRef } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { Pencil, Trash2 } from 'lucide-react';

interface TagContextMenuProps {
  open: boolean;
  x: number;
  y: number;
  onClose: () => void;
  onEdit: () => void;
  onDelete: () => void;
}

export default function TagContextMenu({ open, x, y, onClose, onEdit, onDelete }: TagContextMenuProps) {
  const menuRef = useRef<HTMLDivElement>(null);

  // Close on click outside
  useEffect(() => {
    if (!open) return;
    const handler = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        onClose();
      }
    };
    // Use setTimeout to avoid the same right-click event closing the menu
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

  // Adjust position to keep menu in viewport
  const adjustedPosition = () => {
    const menuWidth = 160;
    const menuHeight = 80;
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
          className="fixed z-[60] min-w-[160px] rounded-md border border-[var(--border)] bg-[var(--bg-elevated)] py-1 shadow-xl"
          style={adjustedPosition()}
        >
          <button
            onClick={() => {
              onEdit();
              onClose();
            }}
            className="flex w-full items-center gap-2.5 px-3 py-1.5 text-left text-[13px] text-[var(--text-secondary)] transition-colors hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"
          >
            <Pencil size={13} />
            Edit Tag
          </button>
          <button
            onClick={() => {
              onDelete();
              onClose();
            }}
            className="flex w-full items-center gap-2.5 px-3 py-1.5 text-left text-[13px] text-[var(--destructive)] transition-colors hover:bg-[var(--destructive)]/8"
          >
            <Trash2 size={13} />
            Delete Tag
          </button>
        </motion.div>
      )}
    </AnimatePresence>
  );
}



