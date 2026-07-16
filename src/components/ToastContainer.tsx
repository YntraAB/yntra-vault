import { AnimatePresence, motion } from 'framer-motion';
import { Check, X, AlertCircle } from 'lucide-react';
import { useAppState } from '@/contexts/AppStateContext';

export default function ToastContainer() {
  const { toasts, removeToast } = useAppState();

  return (
    <div className="fixed right-4 top-4 z-[60] flex flex-col gap-2">
      <AnimatePresence>
        {toasts.map((toast) => (
          <motion.div
            key={toast.id}
            initial={{ opacity: 0, y: -8, scale: 0.98 }}
            animate={{ opacity: 1, y: 0, scale: 1 }}
            exit={{ opacity: 0, x: 20, scale: 0.98 }}
            transition={{ duration: 0.15 }}
            className="flex min-w-[240px] items-center gap-2.5 rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] px-3.5 py-2.5 shadow-sm"
          >
            {toast.type === 'success' && <Check size={14} className="text-[var(--success)]" />}
            {toast.type === 'error' && <X size={14} className="text-[var(--destructive)]" />}
            {toast.type === 'info' && <AlertCircle size={14} className="text-[var(--text-secondary)]" />}
            <span className="flex-1 text-[12px] text-[var(--text-primary)]">{toast.message}</span>
            <button
              onClick={() => removeToast(toast.id)}
              className="text-[var(--text-tertiary)] transition-colors hover:text-[var(--text-primary)]"
            >
              <X size={12} />
            </button>
          </motion.div>
        ))}
      </AnimatePresence>
    </div>
  );
}

