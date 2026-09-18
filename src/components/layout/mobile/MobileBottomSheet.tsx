import React from 'react';
import { motion, AnimatePresence, useDragControls } from 'framer-motion';
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
  const dragControls = useDragControls();

  return (
    <AnimatePresence>
      {open && (
        <div className="fixed inset-0 z-50 flex flex-col justify-end">
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
          dragListener={false}
          dragControls={dragControls}
          dragConstraints={{ top: 0 }}
          dragElastic={0.2}
          onDragEnd={(_, info) => {
            if (info.offset.y > 100 || info.velocity.y > 500) {
              onClose();
            }
          }}
          className="relative z-10 flex max-h-[85vh] w-full flex-col rounded-t-[20px] border-t border-[var(--border)] bg-[var(--bg-elevated)] pb-[calc(env(safe-area-inset-bottom,0px)+16px)] text-[var(--text-primary)] shadow-2xl"
        >
          {/* Drag Handle Indicator */}
          <div
            onPointerDown={(e) => dragControls.start(e)}
            className="flex w-full items-center justify-center pt-3 pb-2 cursor-grab active:cursor-grabbing touch-none"
          >
            <div className="h-1.5 w-12 rounded-full bg-[var(--border-focus)]/50" />
          </div>

          {/* Sheet Header */}
          {title && (
            <div className="flex items-center justify-between border-b border-[var(--border-subtle)] px-4 py-3 shrink-0">
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
          <div className="flex-1 min-h-0 overflow-y-auto px-4 pt-3 touch-pan-y overscroll-contain">
            {children}
          </div>
        </motion.div>
      </div>
      )}
    </AnimatePresence>
  );
}

export default MobileBottomSheet;
