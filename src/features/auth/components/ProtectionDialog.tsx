import { useEffect, useLayoutEffect, useRef, type ReactNode } from 'react';
import { createPortal } from 'react-dom';
import { motion } from 'framer-motion';
import { X } from 'lucide-react';
import { useTranslation } from '@/contexts/LanguageContext';

export const protectionButton = 'inline-flex h-8 shrink-0 items-center justify-center gap-1.5 rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] px-3 text-[12px] font-medium text-[var(--text-secondary)] transition-colors hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)] disabled:cursor-not-allowed disabled:opacity-50';
export const protectionPrimary = 'inline-flex h-8 items-center justify-center gap-1.5 rounded-[3px] bg-[var(--text-primary)] px-3.5 text-[12px] font-semibold text-[var(--bg-base)] transition-opacity hover:opacity-90 disabled:cursor-not-allowed disabled:opacity-40';

export function ProtectionPanel({ title, subtitle, icon, children, footer, onClose }: {
  title: string; subtitle?: string; icon: ReactNode; children: ReactNode; footer: ReactNode; onClose?: () => void;
}) {
  const { t } = useTranslation();
  return <div className="flex max-h-[calc(100dvh-1.5rem)] min-w-0 flex-col overflow-hidden rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] text-[var(--text-primary)] shadow-xl sm:max-h-[85vh]">
    <header className="flex shrink-0 items-center justify-between gap-3 border-b border-[var(--border-subtle)] bg-[var(--bg-base)] px-5 py-3.5">
      <div className="flex min-w-0 items-center gap-2.5">
        <div className="flex h-7 w-7 shrink-0 items-center justify-center rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] text-[var(--text-secondary)]">{icon}</div>
        <div className="min-w-0"><h2 className="text-[14px] font-medium leading-tight">{title}</h2>{subtitle && <p className="truncate text-[11px] text-[var(--text-tertiary)]">{subtitle}</p>}</div>
      </div>
      {onClose && <button type="button" onClick={onClose} aria-label={t('common.close')} className="rounded-[3px] p-1 text-[var(--text-tertiary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"><X size={15}/></button>}
    </header>
    <div className="min-h-0 overflow-y-auto overscroll-contain p-5 text-[12px] leading-relaxed">{children}</div>
    <footer className="flex shrink-0 flex-wrap items-center justify-between gap-2 border-t border-[var(--border-subtle)] bg-[var(--bg-base)] px-5 py-3">{footer}</footer>
  </div>;
}

export function ProtectionDialog({ children, label, onClose }: { children: ReactNode; label: string; onClose?: () => void }) {
  const dialog = useRef<HTMLDivElement>(null);
  const close = useRef(onClose);
  useLayoutEffect(() => { close.current = onClose; }, [onClose]);
  useEffect(() => {
    const previous = document.activeElement as HTMLElement | null;
    const overflow = document.body.style.overflow;
    document.body.style.overflow = 'hidden';
    dialog.current?.focus();
    const trap = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        event.preventDefault(); event.stopImmediatePropagation(); close.current?.();
      }
      if (event.key !== 'Tab') return;
      const controls = dialog.current?.querySelectorAll<HTMLElement>('button:not(:disabled),input:not(:disabled),select:not(:disabled),summary,[tabindex="0"]');
      if (!controls?.length) { event.preventDefault(); return; }
      const first = controls[0], last = controls[controls.length - 1];
      if (!dialog.current?.contains(document.activeElement) || document.activeElement === dialog.current) {
        event.preventDefault(); (event.shiftKey ? last : first).focus();
      } else if (event.shiftKey && document.activeElement === first) { event.preventDefault(); last.focus(); }
      else if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first.focus(); }
    };
    document.addEventListener('keydown', trap, true);
    return () => {
      document.removeEventListener('keydown', trap, true);
      document.body.style.overflow = overflow;
      if (previous?.isConnected) previous.focus();
    };
  }, []);
  return createPortal(<motion.div initial={{ opacity: 0 }} animate={{ opacity: 1 }} className="fixed inset-0 z-[100] flex items-start justify-center overflow-y-auto bg-black/50 p-3 sm:items-center sm:p-4" onClick={() => close.current?.()}>
    <motion.div initial={{ opacity: 0, scale: 0.97, y: 6 }} animate={{ opacity: 1, scale: 1, y: 0 }} transition={{ duration: 0.15 }} ref={dialog} tabIndex={-1} role="dialog" aria-modal="true" aria-label={label} className="my-auto w-full max-w-[420px] outline-none" onClick={event => event.stopPropagation()}>{children}</motion.div>
  </motion.div>, document.body);
}
