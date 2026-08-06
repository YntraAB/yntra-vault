/**
 * PasswordStrength — Real-time password strength indicator
 * 
 * Shows entropy bars, crack time, and warnings.
 * Updates on every keystroke via the Rust analyzer.
 */

import React, { useEffect, useState } from 'react';
import { usePasswordGenerator } from '../lib/useBackend';
import type { StrengthScore, StrengthLevel } from '../lib/backend';

interface PasswordStrengthProps {
  password: string;
  compact?: boolean;
  showWarnings?: boolean;
}

const LEVEL_CONFIG: Record<StrengthLevel, { color: string; label: string; width: string }> = {
  Critical: { color: 'var(--strength-critical, #ef4444)', label: 'Critical', width: '10%' },
  Weak: { color: 'var(--strength-weak, #f59e0b)', label: 'Weak', width: '30%' },
  Fair: { color: 'var(--strength-fair, #eab308)', label: 'Fair', width: '50%' },
  Strong: { color: 'var(--strength-strong, #22c55e)', label: 'Strong', width: '75%' },
  Excellent: { color: 'var(--strength-excellent, #06b6d4)', label: 'Excellent', width: '100%' },
};

export const PasswordStrength: React.FC<PasswordStrengthProps> = ({
  password,
  compact = false,
  showWarnings = true,
}) => {
  const { analyzeStrength } = usePasswordGenerator();
  const [score, setScore] = useState<StrengthScore | null>(null);
  const [debounceTimer, setDebounceTimer] = useState<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    if (!password) {
      setScore(null);
      return;
    }

    // Debounce 150ms for keystroke performance
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

  if (compact) {
    return (
      <div className="flex items-center gap-2">
        <div className="h-1.5 flex-1 rounded-full bg-[var(--bg-elevated)]">
          <div
            className="h-full rounded-full transition-all duration-300"
            style={{ width: config.width, backgroundColor: config.color }}
          />
        </div>
        <span className="text-[11px] font-medium" style={{ color: config.color }}>
          {config.label}
        </span>
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-1.5">
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
          {config.label}
        </span>
      </div>

      {/* Stats */}
      <div className="flex items-center justify-between text-[11px] text-[var(--text-tertiary)]">
        <span>{score.entropy_bits.toFixed(0)} bits entropy</span>
        <span>Crack time: {score.crack_time}</span>
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



