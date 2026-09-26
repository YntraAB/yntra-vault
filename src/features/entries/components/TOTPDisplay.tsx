/**
 * TOTPDisplay — Live TOTP code with countdown ring
 * 
 * Shows the current code with a visual countdown timer.
 * Auto-refreshes when the period expires.
 */

import React, { useState } from 'react';
import { useTotp } from '../hooks/useTotp';
import { useTranslation } from '@/contexts/LanguageContext';
import { Copy, Check } from 'lucide-react';
import { ActionTooltip } from '@/components/ui/tooltip';
import { getBackend, isTauri } from '@/lib/backend';

export interface TOTPDisplayProps {
  secret: string;
  compact?: boolean;
}

export const TOTPDisplay: React.FC<TOTPDisplayProps> = ({
  secret,
  compact = false,
}) => {
  const code = useTotp(secret);
  const { t } = useTranslation();
  const [copied, setCopied] = useState(false);

  if (!code) {
    return (
      <div className="flex items-center gap-3 rounded-[3px] border border-[var(--border-subtle)] bg-[var(--bg-elevated)] px-2.5 py-2">
        <div className="h-4 w-4 animate-spin rounded-full border-2 border-[var(--text-tertiary)] border-t-transparent shrink-0" />
        <span className="text-[12px] text-[var(--text-tertiary)]">{t('totp.generating_code')}</span>
      </div>
    );
  }

  const progress = code.seconds_remaining / code.period;
  const isUrgent = code.seconds_remaining <= 5;

  const handleCopy = async () => {
    try {
      if (isTauri()) {
        const backend = await getBackend();
        await backend.copyToClipboard(code.code, true, 30);
      } else {
        await navigator.clipboard.writeText(code.code);
      }
    } catch {
      return;
    }
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  // Format code with space in middle: "123 456"
  const formattedCode = code.code.length === 6
    ? `${code.code.slice(0, 3)} ${code.code.slice(3)}`
    : code.code;

  if (compact) {
    return (
      <ActionTooltip content={copied ? t('totp.copied') : t('totp.click_to_copy')}>
        <button
          type="button"
          onClick={handleCopy}
          className="flex items-center gap-1.5 rounded-[3px] px-2 py-1 transition-colors hover:bg-[var(--bg-elevated)] select-none"
        >
          <CountdownRing progress={progress} size={14} urgent={isUrgent} />
          <span className={`font-mono text-[13px] font-semibold ${
            isUrgent ? 'text-[var(--text-secondary)]' : 'text-[var(--text-primary)]'
          }`}>
            {formattedCode}
          </span>
          {copied && <Check size={12} className="text-[var(--text-primary)]" />}
        </button>
      </ActionTooltip>
    );
  }

  return (
    <div className="flex items-center gap-3 rounded-[3px] border border-[var(--border-subtle)] bg-[var(--bg-elevated)] px-2.5 py-2 transition-all hover:bg-[var(--bg-hover)] select-none">
      <CountdownRing progress={progress} size={18} urgent={isUrgent} />

      <div className="flex items-baseline gap-2">
        <span className={`font-mono text-[16px] font-bold tracking-wider select-all ${
          isUrgent ? 'text-[var(--text-secondary)]' : 'text-[var(--text-primary)]'
        }`}>
          {formattedCode}
        </span>
        <span className="text-[10px] text-[var(--text-tertiary)] font-medium select-none">
          {t('totp.seconds_remaining', { seconds: code.seconds_remaining })}
        </span>
      </div>

      <ActionTooltip content={copied ? t('totp.copied') : t('totp.copy')}>
        <button
          type="button"
          onClick={handleCopy}
          className="ml-auto rounded-[3px] p-1 transition-colors hover:bg-[var(--bg-active)] select-none"
        >
          {copied
            ? <Check size={13} className="text-[var(--text-primary)]" />
            : <Copy size={13} className="text-[var(--text-secondary)]" />
          }
        </button>
      </ActionTooltip>
    </div>
  );
};

// ─── Countdown Ring ─────────────────────────────────────────────────────

const CountdownRing: React.FC<{
  progress: number; // 0-1
  size: number;
  urgent: boolean;
}> = ({ progress, size, urgent }) => {
  const r = size / 2 - 2;
  const circumference = 2 * Math.PI * r;
  const strokeDashoffset = circumference * (1 - progress);
  const color = urgent ? 'var(--text-secondary)' : 'var(--accent, #e8e8e8)';

  return (
    <svg width={size} height={size} className="shrink-0 -rotate-90">
      <circle
        cx={size / 2}
        cy={size / 2}
        r={r}
        fill="none"
        stroke="var(--bg-elevated)"
        strokeWidth={2}
      />
      <circle
        cx={size / 2}
        cy={size / 2}
        r={r}
        fill="none"
        stroke={color}
        strokeWidth={2}
        strokeDasharray={circumference}
        strokeDashoffset={strokeDashoffset}
        strokeLinecap="round"
        className="transition-all duration-1000 linear"
      />
    </svg>
  );
};

export default TOTPDisplay;
