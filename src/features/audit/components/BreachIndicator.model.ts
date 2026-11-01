import type { BreachStatus } from '@/lib/backend';


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