/**
 * TOTPDisplay — Live TOTP code with countdown ring
 * 
 * Shows the current code with a visual countdown timer.
 * Auto-refreshes when the period expires.
 */

import React, { useState } from 'react';
import { useTotp } from '../lib/useBackend';
import { Copy, Check } from 'lucide-react';

interface TOTPDisplayProps {
  secret: string;
  compact?: boolean;
}

export const TOTPDisplay: React.FC<TOTPDisplayProps> = ({
  secret,
  compact = false,
}) => {
  const code = useTotp(secret);
  const [copied, setCopied] = useState(false);

  if (!code) {
    return (
      <div className="flex items-center gap-3 rounded-[3px] border border-[var(--border-subtle)] bg-[var(--bg-elevated)] px-2.5 py-2">
        <div className="h-4 w-4 animate-spin rounded-full border-2 border-[var(--text-tertiary)] border-t-transparent shrink-0" />
        <span className="text-[12px] text-[var(--text-tertiary)]">Generating code...</span>
      </div>
    );
  }

  const progress = code.seconds_remaining / code.period;
  const isUrgent = code.seconds_remaining <= 5;

  const handleCopy = async () => {
    await navigator.clipboard.writeText(code.code);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  // Format code with space in middle: "123 456"
  const formattedCode = code.code.length === 6
    ? `${code.code.slice(0, 3)} ${code.code.slice(3)}`
    : code.code;

  if (compact) {
    return (
      <button
        onClick={handleCopy}
        className="flex items-center gap-1.5 rounded-md px-2 py-1 transition-colors hover:bg-[var(--bg-elevated)]"
        title="Click to copy"
      >
        <CountdownRing progress={progress} size={14} urgent={isUrgent} />
        <span className={`font-mono text-[13px] font-semibold ${
          isUrgent ? 'text-red-400' : 'text-[var(--text-primary)]'
        }`}>
          {formattedCode}
        </span>
        {copied && <Check size={12} className="text-green-500" />}
      </button>
    );
  }

  return (
    <div className="flex items-center gap-3 rounded-[3px] border border-[var(--border-subtle)] bg-[var(--bg-elevated)] px-2.5 py-2 transition-all hover:bg-[var(--bg-hover)]">
      <CountdownRing progress={progress} size={18} urgent={isUrgent} />

      <div className="flex items-baseline gap-2">
        <span className={`font-mono text-[16px] font-bold tracking-wider ${
          isUrgent ? 'text-red-400 animate-pulse' : 'text-[var(--text-primary)]'
        }`}>
          {formattedCode}
        </span>
        <span className="text-[10px] text-[var(--text-tertiary)] font-medium">
          {code.seconds_remaining}s remaining
        </span>
      </div>

      <button
        onClick={handleCopy}
        className="ml-auto rounded-[3px] p-1 transition-colors hover:bg-[var(--bg-active)]"
        title="Copy code"
      >
        {copied
          ? <Check size={13} className="text-green-500" />
          : <Copy size={13} className="text-[var(--text-secondary)]" />
        }
      </button>
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
  const color = urgent ? '#ef4444' : 'var(--accent, #3b82f6)';

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



