import { useToast } from '@/contexts/ToastContext';
import { useState, useCallback } from 'react';
import { Copy, Check } from 'lucide-react';
import { useTranslation } from '@/contexts/LanguageContext';
import { useSettings } from '@/contexts/SettingsContext';
import { ActionTooltip } from './tooltip';
import { getBackend, isTauri } from '@/lib/backend';

export interface CopyButtonProps {
  value: string;
  className?: string;
  size?: number;
  label?: string;
  isSensitive?: boolean;
  clearAfterSecs?: number;
}

export function CopyButton({
  value,
  className = '',
  size = 14,
  label,
  isSensitive = true,
  clearAfterSecs,
}: CopyButtonProps) {
  const { t } = useTranslation();
  const { addToast } = useToast();
  const { settings } = useSettings();
  const [copyFailed, setCopyFailed] = useState(false);
  const [copied, setCopied] = useState(false);
  const tooltipLabel = label || t('common.copy');
  const effectiveClearSecs = clearAfterSecs ?? settings.clipboardClearSeconds;

  const handleCopy = useCallback(
    async (e: React.MouseEvent) => {
      e.stopPropagation();
      try {
        if (isTauri()) {
          const backend = await getBackend();
          await backend.copyToClipboard(value, isSensitive, effectiveClearSecs);
        } else {
          await navigator.clipboard.writeText(value);
        }
      } catch {
        setCopyFailed(true);
        addToast({ message: t('toast.copy_failed'), type: 'error' });
        return;
      }
      setCopyFailed(false);
      setCopied(true);
      setTimeout(() => setCopied(false), 800);
    },
    [value, isSensitive, effectiveClearSecs, addToast, t]
  );

  return (
    <ActionTooltip content={copyFailed ? t('toast.copy_failed') : copied ? t('common.copied') : tooltipLabel}>
      <button
        type="button"
        aria-label={copyFailed ? t('toast.copy_failed') : tooltipLabel}
        onClick={handleCopy}
        className={`inline-flex items-center justify-center rounded-[3px] p-1.5 sm:p-1 text-[var(--text-tertiary)] transition-all duration-100 hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)] active:scale-95 select-none ${className}`}
      >
        {copied ? (
          <Check size={size} className="text-[var(--success)]" />
        ) : (
          <Copy size={size} />
        )}
      </button>
    </ActionTooltip>
  );
}

export default CopyButton;
