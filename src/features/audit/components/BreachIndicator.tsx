import React, { useEffect, useState, useCallback, useRef } from 'react';
import { useBackend } from '@/lib/useBackend';
import { useSettings } from '@/features/settings';
import { useTranslation } from '@/contexts/LanguageContext';
import type { BreachStatus } from '@/lib/backend';
import { ActionTooltip } from '@/components/ui/tooltip';
import { useSecurityAudit } from '../hooks/useSecurityAudit';

export interface BreachIndicatorProps {
  status?: BreachStatus;
  password?: string;
  entryId?: string;
  isReused?: boolean;
  reusedServices?: string;
  isWeak?: boolean;
  compact?: boolean;
  onStatusChange?: (status: BreachStatus) => void;
  hideIfSafe?: boolean;
}

export const BreachIndicator: React.FC<BreachIndicatorProps> = ({
  status: initialStatus,
  password,
  entryId,
  isReused: propIsReused,
  reusedServices: propReusedServices,
  isWeak: propIsWeak,
  compact = false,
  onStatusChange,
  hideIfSafe = false,
}) => {
  const { backend } = useBackend();
  const { settings } = useSettings();
  const { t } = useTranslation();
  const { audit, runAudit } = useSecurityAudit();
  const [status, setStatus] = useState<BreachStatus>(
    initialStatus || { type: 'Unknown' }
  );
  const lastCheckedPasswordRef = useRef<string | null>(null);

  useEffect(() => {
    if (!audit && entryId) {
      runAudit(true, true);
    }
  }, [audit, entryId, runAudit]);

  const entryReusedIssue = entryId
    ? audit?.issues.find((i) => i.entry_id === entryId && i.issue_type === 'ReusedPassword')
    : undefined;
  const entryWeakIssue = entryId
    ? audit?.issues.find((i) => i.entry_id === entryId && i.issue_type === 'WeakPassword')
    : undefined;

  const effectiveReused = propIsReused ?? !!entryReusedIssue;
  const effectiveWeak = propIsWeak ?? !!entryWeakIssue;
  const effectiveReusedServices =
    propReusedServices ||
    (entryReusedIssue ? entryReusedIssue.description.split(': ')[1] : undefined);

  useEffect(() => {
    if (initialStatus) {
      setStatus(initialStatus);
      if (initialStatus.type !== 'Unknown') {
        lastCheckedPasswordRef.current = password || null;
      }
    }
  }, [initialStatus, password]);

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

  const config = getStatusConfig(status, t, {
    isReused: effectiveReused,
    reusedServices: effectiveReusedServices,
    isWeak: effectiveWeak,
  });

  const trulySafe = (status.type === 'Safe' || status.type === 'Unknown') && !effectiveReused && !effectiveWeak;

  if (hideIfSafe && trulySafe) {
    return null;
  }

  if (compact) {
    return (
      <ActionTooltip content={config.tooltip}>
        <span className={`text-[10px] font-medium tracking-wide select-none ${config.textColor}`}>
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

export interface StatusConfig {
  shortLabel: string;
  label: string;
  detail?: string;
  tooltip: string;
  textColor: string;
}

export function getStatusConfig(
  status: BreachStatus,
  t: (key: string, params?: Record<string, string | number>) => string,
  extra?: { isReused?: boolean; reusedServices?: string; isWeak?: boolean }
): StatusConfig {
  if (extra?.isReused && status.type !== 'Breached' && status.type !== 'Checking') {
    return {
      shortLabel: t('breach.reused_status'),
      label: status.type === 'Safe' ? t('breach.no_breaches_reused') : t('breach.reused_status'),
      detail: extra.reusedServices ? t('security.desc_reused_with', { services: extra.reusedServices }) : undefined,
      tooltip: t('breach.reused_tooltip'),
      textColor: 'text-purple-400 font-medium',
    };
  }

  if (extra?.isWeak && status.type !== 'Breached' && status.type !== 'Checking') {
    return {
      shortLabel: t('security.stat_weak'),
      label: status.type === 'Safe' ? t('breach.no_breaches_weak') : t('security.stat_weak'),
      tooltip: t('security.desc_weak'),
      textColor: 'text-amber-400 font-medium',
    };
  }

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

    case 'Breached': {
      const formatted = formatCount(status.breach_count);
      return {
        shortLabel: t('breach.count_short', { count: formatted }),
        label: t('breach.found_in_breaches', { count: status.breach_count.toLocaleString() }),
        tooltip: t('breach.warning_breached', { count: status.breach_count.toLocaleString() }),
        textColor: 'text-red-500 font-semibold',
      };
    }

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


export function formatCount(count: number): string {
  if (count >= 1_000_000) return `${(count / 1_000_000).toFixed(1)}M`;
  if (count >= 1_000) return `${(count / 1_000).toFixed(1)}K`;
  return count.toString();
}

export default BreachIndicator;
