import React from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { X } from 'lucide-react';

export interface MobileBottomSheetProps {
  open: boolean;
  onClose: () => void;
  title?: string;
  children: React.ReactNode;
}

export function MobileBottomSheet({
  open,
  onClose,
  title,
  children,
}: MobileBottomSheetProps) {
  return (
    <AnimatePresence>
      {open && (
        <div className="fixed inset-0 z-50 flex flex-col justify-end select-none">
        {/* Backdrop */}
        <motion.div
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          onClick={onClose}
          className="fixed inset-0 bg-black/60 backdrop-blur-xs select-none"
        />

        {/* Sheet Content */}
        <motion.div
          initial={{ y: '100%' }}
          animate={{ y: 0 }}
          exit={{ y: '100%' }}
          transition={{ type: 'spring', damping: 25, stiffness: 300 }}
          drag="y"
          dragConstraints={{ top: 0 }}
          dragElastic={0.2}
          onDragEnd={(_, info) => {
            if (info.offset.y > 100 || info.velocity.y > 500) {
              onClose();
            }
          }}
          className="relative z-10 flex max-h-[85vh] w-full flex-col rounded-t-[20px] border-t border-[var(--border)] bg-[var(--bg-elevated)] pb-[calc(env(safe-area-inset-bottom,0px)+16px)] text-[var(--text-primary)] shadow-2xl select-none"
        >
          {/* Drag Handle Indicator */}
          <div className="flex w-full items-center justify-center pt-3 pb-1">
            <div className="h-1.5 w-12 rounded-full bg-[var(--border-focus)]/50" />
          </div>

          {/* Sheet Header */}
          {title && (
            <div className="flex items-center justify-between border-b border-[var(--border-subtle)] px-4 py-3">
              <h3 className="text-[16px] font-semibold tracking-tight">{title}</h3>
              <button
                onClick={onClose}
                className="flex h-8 w-8 items-center justify-center rounded-full text-[var(--text-tertiary)] active:bg-[var(--bg-hover)] active:text-[var(--text-primary)] cursor-pointer"
              >
                <X size={18} />
              </button>
            </div>
          )}

          {/* Sheet Body */}
          <div className="flex-1 overflow-y-auto px-4 pt-3">
            {children}
          </div>
        </motion.div>
      </div>
      )}
    </AnimatePresence>
  );
}

export default MobileBottomSheet;
