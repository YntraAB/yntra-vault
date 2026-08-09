/**
 * BreachIndicator — Shows if a password has been found in data breaches
 * 
 * States: unknown → checking → safe/breached/error
 * Uses HIBP k-anonymity API via Rust backend.
 * Automatically runs checks on password change with debouncing.
 */

import React, { useEffect, useState, useCallback, useRef } from 'react';
import { useBackend } from '../lib/useBackend';
import { useAppState } from '../contexts/AppStateContext';
import { useTranslation } from '../contexts/LanguageContext';
import type { BreachStatus } from '../lib/backend';
import { ActionTooltip } from './ui/tooltip';

interface BreachIndicatorProps {
  /** Pre-fetched breach status from entry data */
  status?: BreachStatus;
  /** Password to check (triggers API call) */
  password?: string;
  /** Show as compact text or full detail */
  compact?: boolean;
  /** Callback when status changes */
  onStatusChange?: (status: BreachStatus) => void;
  /** Hide indicator if password is safe or unknown */
  hideIfSafe?: boolean;
}

export const BreachIndicator: React.FC<BreachIndicatorProps> = ({
  status: initialStatus,
  password,
  compact = false,
  onStatusChange,
  hideIfSafe = false,
}) => {
  const { backend } = useBackend();
  const { settings } = useAppState();
  const { t } = useTranslation();
  const [status, setStatus] = useState<BreachStatus>(
    initialStatus || { type: 'Unknown' }
  );
  const lastCheckedPasswordRef = useRef<string | null>(null);

  // Update status if initialStatus prop changes
  useEffect(() => {
    if (initialStatus) {
      setStatus(initialStatus);
      if (initialStatus.type !== 'Unknown') {
        lastCheckedPasswordRef.current = password || null;
      }
    }
  }, [initialStatus, password]);

  // Check breach logic
  const checkBreach = useCallback(async (targetPassword: string) => {
    if (!backend || !targetPassword || targetPassword.length < 4) {
      setStatus({ type: 'Unknown' });
      return;
    }

    setStatus({ type: 'Checking' });

    try {
      const result = await backend.checkPasswordBreach(targetPassword);
      const newStatus: BreachStatus = result.is_breached
        ? { type: 'Breached', breach_count: result.breach_count, checked_at: result.checked_at }
        : { type: 'Safe', checked_at: result.checked_at };

      setStatus(newStatus);
      onStatusChange?.(newStatus);
      lastCheckedPasswordRef.current = targetPassword;
    } catch (e: any) {
      setStatus({ type: 'Error', message: e.toString() });
    }
  }, [backend, onStatusChange]);

  // Auto-trigger check with a 500ms debounce when password changes
  useEffect(() => {
    if (!settings.autoBreachCheck) {
      return;
    }

    if (!password || password.trim() === '') {
      setStatus({ type: 'Unknown' });
      return;
    }

    if (password === lastCheckedPasswordRef.current) {
      return;
    }

    const timer = setTimeout(() => {
      checkBreach(password);
    }, 500);

    return () => clearTimeout(timer);
  }, [password, checkBreach, settings.autoBreachCheck]);

  const config = getStatusConfig(status, t);

  if (hideIfSafe && (status.type === 'Safe' || status.type === 'Unknown')) {
    return null;
  }

  if (compact) {
    return (
      <ActionTooltip content={config.tooltip}>
        <span className={`text-[10px] font-medium tracking-wide ${config.textColor}`}>
          {config.shortLabel}
        </span>
      </ActionTooltip>
    );
  }

  return (
    <div className="flex items-center gap-1.5 text-[11px] font-medium py-0.5 select-none">
      <span className={config.textColor}>{config.label}</span>
      {config.detail && (
        <span className="text-[10px] text-[var(--text-tertiary)] font-normal">({config.detail})</span>
      )}
    </div>
  );
};

// ─── Config Helper ──────────────────────────────────────────────────────

interface StatusConfig {
  shortLabel: string;
  label: string;
  detail?: string;
  tooltip: string;
  textColor: string;
}

function getStatusConfig(status: BreachStatus, t: (key: string, params?: Record<string, string | number>) => string): StatusConfig {
  switch (status.type) {
    case 'Unknown':
      return {
        shortLabel: t('breach.not_checked'),
        label: t('breach.status_not_checked'),
        tooltip: t('breach.tooltip_not_checked'),
        textColor: 'text-[var(--text-tertiary)]',
      };

    case 'Checking':
      return {
        shortLabel: t('breach.checking'),
        label: t('breach.checking_safety'),
        tooltip: t('breach.tooltip_checking'),
        textColor: 'text-[var(--text-secondary)] animate-pulse',
      };

    case 'Safe':
      return {
        shortLabel: t('breach.safe'),
        label: t('breach.no_breaches'),
        tooltip: t('breach.tooltip_safe'),
        textColor: 'text-green-500',
      };

    case 'Breached':
      const formatted = formatCount(status.breach_count);
      return {
        shortLabel: t('breach.count_short', { count: formatted }),
        label: t('breach.found_in_breaches', { count: status.breach_count.toLocaleString() }),
        tooltip: t('breach.warning_breached', { count: status.breach_count.toLocaleString() }),
        textColor: 'text-red-500 font-semibold',
      };

    case 'Error':
      return {
        shortLabel: t('breach.check_failed'),
        label: t('breach.failed_verify'),
        detail: t('breach.offline'),
        tooltip: t('breach.error_details', { message: status.message }),
        textColor: 'text-amber-500',
      };
  }
}

function formatCount(count: number): string {
  if (count >= 1_000_000) return `${(count / 1_000_000).toFixed(1)}M`;
  if (count >= 1_000) return `${(count / 1_000).toFixed(1)}K`;
  return count.toString();
}

export default BreachIndicator;



