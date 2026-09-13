import React, { useEffect, useState } from 'react';
import { usePasswordGenerator } from '@/features/generator';
import { useTranslation } from '@/contexts/LanguageContext';
import type { StrengthScore, StrengthLevel } from '@/lib/backend';

export interface PasswordStrengthProps {
  password: string;
  compact?: boolean;
  showWarnings?: boolean;
}

const LEVEL_CONFIG: Record<StrengthLevel, { color: string; labelKey: string; width: string }> = {
  Critical: { color: 'var(--strength-critical, #ef4444)', labelKey: 'strength.critical', width: '10%' },
  Weak: { color: 'var(--strength-weak, #f59e0b)', labelKey: 'strength.weak', width: '30%' },
  Fair: { color: 'var(--strength-fair, #eab308)', labelKey: 'strength.fair', width: '50%' },
  Strong: { color: 'var(--strength-strong, #22c55e)', labelKey: 'strength.strong', width: '75%' },
  Excellent: { color: 'var(--strength-excellent, #e8e8e8)', labelKey: 'strength.excellent', width: '100%' },
};

export const PasswordStrength: React.FC<PasswordStrengthProps> = ({
  password,
  compact = false,
  showWarnings = true,
}) => {
  const { analyzeStrength } = usePasswordGenerator();
  const { t } = useTranslation();
  const [score, setScore] = useState<StrengthScore | null>(null);
  const [debounceTimer, setDebounceTimer] = useState<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    if (!password) {
      setScore(null);
      return;
    }

    if (debounceTimer) clearTimeout(debounceTimer);
    const timer = setTimeout(async () => {
      const result = await analyzeStrength(password);
      if (result) setScore(result);
    }, 150);
    setDebounceTimer(timer);

    return () => clearTimeout(timer);
  }, [password]);

  if (!password || !score) return null;

  const config = LEVEL_CONFIG[score.level];
  const label = t(config.labelKey);

  if (compact) {
    return (
      <div className="flex items-center gap-2 select-none">
        <div className="h-1.5 flex-1 rounded-full bg-[var(--bg-elevated)]">
          <div
            className="h-full rounded-full transition-all duration-300"
            style={{ width: config.width, backgroundColor: config.color }}
          />
        </div>
        <span className="text-[11px] font-medium" style={{ color: config.color }}>
          {label}
        </span>
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-1.5 select-none">
      {/* Strength bar */}
      <div className="flex items-center gap-2">
        <div className="flex flex-1 gap-0.5">
          {(['Critical', 'Weak', 'Fair', 'Strong', 'Excellent'] as StrengthLevel[]).map((level, i) => {
            const levels: StrengthLevel[] = ['Critical', 'Weak', 'Fair', 'Strong', 'Excellent'];
            const currentIdx = levels.indexOf(score.level);
            const isActive = i <= currentIdx;
            return (
              <div
                key={level}
                className="h-1.5 flex-1 rounded-full transition-all duration-300"
                style={{
                  backgroundColor: isActive ? config.color : 'var(--bg-elevated)',
                }}
              />
            );
          })}
        </div>
        <span className="text-[11px] font-medium min-w-[60px] text-right" style={{ color: config.color }}>
          {label}
        </span>
      </div>

      {/* Stats */}
      <div className="flex items-center justify-between text-[11px] text-[var(--text-tertiary)]">
        <span>{t('strength.bits_entropy', { bits: score.entropy_bits.toFixed(0) })}</span>
        <span>{t('strength.crack_time', { time: score.crack_time })}</span>
      </div>

      {/* Warnings */}
      {showWarnings && score.warnings.length > 0 && (
        <div className="flex flex-col gap-0.5 mt-0.5">
          {score.warnings.slice(0, 3).map((warning, i) => (
            <span key={i} className="text-[11px] text-[var(--text-tertiary)]">
              ⚠ {warning}
            </span>
          ))}
        </div>
      )}
    </div>
  );
};

export default PasswordStrength;
