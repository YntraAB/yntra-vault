import { AnimatePresence, motion } from 'framer-motion';
import { Check, X, AlertCircle } from 'lucide-react';
import { useToast } from '@/contexts/ToastContext';

export function ToastContainer() {
  const { toasts, removeToast } = useToast();

  return (
    <div className="pointer-events-none fixed left-4 right-4 top-[max(calc(env(safe-area-inset-top,0px)+0.75rem),3.75rem)] z-[70] flex flex-col items-center gap-2 select-none md:left-auto md:right-4 md:top-4 md:items-end">
      <AnimatePresence>
        {toasts.map((toast) => (
          <motion.div
            key={toast.id}
            initial={{ opacity: 0, y: -8, scale: 0.98 }}
            animate={{ opacity: 1, y: 0, scale: 1 }}
            exit={{ opacity: 0, scale: 0.98 }}
            transition={{ duration: 0.15 }}
            className="pointer-events-auto flex w-full max-w-[420px] sm:w-auto min-w-[240px] items-center gap-2.5 rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] px-3.5 py-2.5 shadow-md backdrop-blur-sm"
          >
            {toast.type === 'success' && <Check size={14} className="shrink-0 text-[var(--success)]" />}
            {toast.type === 'error' && <X size={14} className="shrink-0 text-[var(--destructive)]" />}
            {toast.type === 'info' && <AlertCircle size={14} className="shrink-0 text-[var(--text-secondary)]" />}
            <span className="flex-1 text-[12px] text-[var(--text-primary)] leading-tight">{toast.message}</span>
            <button
              onClick={() => removeToast(toast.id)}
              className="text-[var(--text-tertiary)] transition-colors hover:text-[var(--text-primary)] cursor-pointer p-0.5 shrink-0"
              aria-label="Close notification"
            >
              <X size={12} />
            </button>
          </motion.div>
        ))}
      </AnimatePresence>
    </div>
  );
}

export default ToastContainer;
