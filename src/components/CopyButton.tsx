import { useState, useCallback } from 'react';
import { Copy, Check } from 'lucide-react';
import { useTranslation } from '@/contexts/LanguageContext';
import { ActionTooltip } from './ui/tooltip';

interface CopyButtonProps {
  value: string;
  className?: string;
  size?: number;
  label?: string;
}

export default function CopyButton({ value, className = '', size = 14, label }: CopyButtonProps) {
  const { t } = useTranslation();
  const [copied, setCopied] = useState(false);
  const tooltipLabel = label || t('common.copy');

  const handleCopy = useCallback(
    (e: React.MouseEvent) => {
      e.stopPropagation();
      navigator.clipboard.writeText(value).catch(() => {});
      setCopied(true);
      setTimeout(() => setCopied(false), 800);
    },
    [value]
  );

  return (
    <ActionTooltip content={copied ? t('common.copied') : tooltipLabel}>
      <button
        type="button"
        onClick={handleCopy}
        className={`inline-flex items-center justify-center rounded-[3px] p-1 text-[var(--text-tertiary)] transition-all duration-100 hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)] active:scale-95 ${className}`}
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



