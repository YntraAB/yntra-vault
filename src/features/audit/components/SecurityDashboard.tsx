import React, { useEffect } from 'react';
import { useSecurityAudit } from '../hooks/useSecurityAudit';
import type { SecurityIssue } from '@/lib/backend';
import { useSettings } from '@/features/settings';
import { useEntries } from '@/features/entries';
import { useTranslation } from '@/contexts/LanguageContext';
import {
  ShieldAlert, ShieldCheck,
  Key, Clock, Copy, Lock, RefreshCw, ChevronRight,
} from 'lucide-react';
import { Skeleton } from '@/components/ui';
import { motion, AnimatePresence } from 'framer-motion';
import { ActionTooltip } from '@/components/ui/tooltip';

export interface SecurityDashboardProps {
  onNavigateToEntry?: (entryId: string) => void;
  onOpenChangePassword?: () => void;
}

export const SecurityDashboard: React.FC<SecurityDashboardProps> = ({
  onNavigateToEntry,
  onOpenChangePassword,
}) => {
  const { t } = useTranslation();
  const { audit, loading, runAudit } = useSecurityAudit();
  const { settings } = useSettings();
  const { entries } = useEntries();
  const disableDelays = settings.disableSkeletonDelays;

  useEffect(() => {
    runAudit(disableDelays);
  }, [runAudit, disableDelays]);

  // Silently refresh the audit data in real-time as background checks complete
  useEffect(() => {
    if (!audit) return;
    runAudit(disableDelays, true);
  }, [runAudit, disableDelays, entries]);

  const score = audit?.health_score ?? 0;
  const scoreColor = score >= 80 ? '#10b981' : score >= 50 ? '#f59e0b' : '#ef4444';

  // Group reciprocal reused password issues into unified cluster items
  const displayIssues: DisplaySecurityIssue[] = [];
  if (audit?.issues) {
    const seenReuseKeys = new Set<string>();

    for (const issue of audit.issues) {
      if (issue.issue_type === 'ReusedPassword') {
        const parts = issue.description.split(': ');
        const otherTitles = parts.length > 1 ? parts[1].split(',').map((s) => s.trim()) : [];
        const allTitles = Array.from(new Set([issue.entry_title.trim(), ...otherTitles])).sort((a, b) => a.localeCompare(b));
        const groupKey = allTitles.map((t) => t.toLowerCase()).join(':::');

        if (seenReuseKeys.has(groupKey)) {
          continue;
        }
        seenReuseKeys.add(groupKey);

        const clusterIssues = audit.issues.filter(
          (i) => i.issue_type === 'ReusedPassword' && allTitles.some((t) => t.toLowerCase() === i.entry_title.toLowerCase())
        );

        const groupEntries = allTitles.map((title) => {
          const matchingIssue = clusterIssues.find((i) => i.entry_title.toLowerCase() === title.toLowerCase());
          return {
            id: matchingIssue ? matchingIssue.entry_id : issue.entry_id,
            title,
          };
        });

        displayIssues.push({
          id: `reused-group-${groupKey}`,
          issue_type: 'ReusedPassword',
          severity: issue.severity,
          entry_id: issue.entry_id,
          entry_title: allTitles.join(', '),
          description: issue.description,
          is_group: true,
          group_entries: groupEntries,
        });
      } else {
        displayIssues.push({
          id: `${issue.entry_id}-${issue.issue_type}`,
          issue_type: issue.issue_type,
          severity: issue.severity,
          entry_id: issue.entry_id,
          entry_title: issue.entry_title,
          description: issue.description,
          is_group: false,
        });
      }
    }
  }

  return (
    <div className="flex flex-col gap-3 select-none">
      <AnimatePresence mode="wait">
        {loading && !audit ? (
          <motion.div
            key="audit-skeleton"
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            transition={{ duration: 0.1, ease: 'easeInOut' }}
            className="flex flex-col gap-3"
          >
            {/* Health Score Skeleton */}
            <div className="flex items-center gap-3.5 rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] p-3">
              <Skeleton className="h-10 w-10 rounded-full shrink-0" />
              <div className="flex flex-col gap-1.5 flex-1 min-w-0">
                <Skeleton className="h-3.5 w-24 rounded" />
                <Skeleton className="h-2.5 w-32 rounded" />
              </div>
              <Skeleton className="h-7 w-7 rounded-[3px] shrink-0" />
            </div>

            {/* Issue Summary Cards Skeleton */}
            <div className="grid grid-cols-2 gap-2">
              {[...Array(6)].map((_, i) => (
                <div key={i} className="flex items-center gap-2.5 rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] px-3 py-2">
                  <Skeleton className="h-6 w-6 rounded-[3px] shrink-0" />
                  <div className="flex flex-col gap-1 flex-1 min-w-0">
                    <Skeleton className="h-3.5 w-8 rounded" />
                    <Skeleton className="h-2.5 w-16 rounded" />
                  </div>
                </div>
              ))}
            </div>

            {/* Issues List Skeleton */}
            <div className="flex flex-col gap-1.5 mt-1">
              <Skeleton className="h-3 w-20 rounded" />
              <div className="flex flex-col rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] divide-y divide-[var(--border-subtle)] overflow-hidden">
                {[...Array(3)].map((_, i) => (
                  <div key={i} className="flex items-center gap-2.5 px-3 py-2">
                    <Skeleton className="h-1.5 w-1.5 rounded-full shrink-0" />
                    <div className="flex-1 flex flex-col gap-1 min-w-0">
                      <Skeleton className="h-3 w-24 rounded" />
                      <Skeleton className="h-2.5 w-36 rounded" />
                    </div>
                  </div>
                ))}
              </div>
            </div>
          </motion.div>
        ) : !audit ? null : (
          <motion.div
            key="audit-content"
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            transition={{ duration: 0.1, ease: 'easeInOut' }}
            className="flex flex-col gap-3"
          >
            {/* Health Score - Compact Meter with Color Accent */}
            <div className="flex items-center gap-3.5 rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] p-3">
              <div className="relative flex h-10 w-10 shrink-0 items-center justify-center">
                <svg viewBox="0 0 36 36" className="h-10 w-10 -rotate-90">
                  <circle
                    cx="18" cy="18" r="15"
                    fill="none"
                    stroke="var(--border)"
                    strokeWidth="2.5"
                  />
                  <circle
                    cx="18" cy="18" r="15"
                    fill="none"
                    stroke={scoreColor}
                    strokeWidth="2.5"
                    strokeDasharray={`${score} 100`}
                    strokeLinecap="round"
                    className="transition-all duration-700"
                  />
                </svg>
                <span
                  className="absolute text-[12px] font-bold font-mono"
                  style={{ color: scoreColor }}
                >
                  {score}
                </span>
              </div>

              <div className="flex flex-col gap-0.5">
                <h3 className="text-[13px] font-medium text-[var(--text-primary)]">
                  {t('security.health_score')}
                </h3>
                <p className="text-[11px] text-[var(--text-tertiary)]">
                  {t('security.passwords_analyzed', { count: audit.total_entries })}
                </p>
              </div>

              <ActionTooltip content={t('security.refresh_tooltip')}>
                <button
                  type="button"
                  onClick={() => runAudit(disableDelays)}
                  className="ml-auto flex h-7 w-7 items-center justify-center rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:bg-[var(--bg-hover)] transition-colors cursor-pointer shrink-0"
                >
                  <RefreshCw size={13} className={loading ? 'animate-spin' : ''} />
                </button>
              </ActionTooltip>
            </div>

            {/* Issue Summary Cards */}
            <div className="grid grid-cols-2 gap-2">
              <StatCard
                icon={<ShieldAlert size={14} />}
                label={t('security.stat_breached')}
                count={audit.breached_count}
                iconClass="border border-[var(--border)] bg-[var(--bg-base)] text-[var(--text-secondary)]"
              />
              <StatCard
                icon={<Key size={14} />}
                label={t('security.stat_weak')}
                count={audit.weak_count}
                iconClass="border border-[var(--border)] bg-[var(--bg-base)] text-[var(--text-secondary)]"
              />
              <StatCard
                icon={<Copy size={14} />}
                label={t('security.stat_reused')}
                count={audit.reused_count}
                iconClass="border border-[var(--border)] bg-[var(--bg-base)] text-[var(--text-secondary)]"
              />
              <StatCard
                icon={<Clock size={14} />}
                label={t('security.stat_old')}
                count={audit.old_count}
                iconClass="border border-[var(--border)] bg-[var(--bg-base)] text-[var(--text-secondary)]"
              />
              <StatCard
                icon={<Lock size={14} />}
                label={t('security.stat_missing_2fa')}
                count={audit.no_2fa_count}
                iconClass="border border-[var(--border)] bg-[var(--bg-base)] text-[var(--text-secondary)]"
              />
              <StatCard
                icon={<ShieldCheck size={14} />}
                label={t('security.stat_secure')}
                count={audit.total_entries - audit.breached_count - audit.weak_count}
                iconClass="border border-[var(--border)] bg-[var(--bg-base)] text-[var(--text-secondary)]"
              />
            </div>

            {/* Issues List */}
            {displayIssues.length > 0 && (
              <div className="flex flex-col gap-1.5 mt-1">
                <span className="text-[11px] font-semibold uppercase tracking-wider text-[var(--text-tertiary)]">
                  {t('security.issues_count', { count: displayIssues.length })}
                </span>
                <div className="flex flex-col rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] divide-y divide-[var(--border-subtle)] overflow-hidden">
                  {displayIssues.map((issue) => (
                    <IssueRow
                      key={issue.id}
                      issue={issue}
                      onNavigateToEntry={onNavigateToEntry}
                      onClick={() => {
                        if (issue.entry_id === 'master_password') {
                          onOpenChangePassword?.();
                        } else if (issue.is_group && issue.group_entries && issue.group_entries.length > 0) {
                          onNavigateToEntry?.(issue.group_entries[0].id);
                        } else {
                          onNavigateToEntry?.(issue.entry_id);
                        }
                      }}
                    />
                  ))}
                </div>
              </div>
            )}

            {displayIssues.length === 0 && (
              <div className="flex flex-col items-center gap-1.5 rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] p-4 text-center">
                <ShieldCheck size={22} className="text-[var(--text-secondary)]" />
                <span className="text-[12px] font-medium text-[var(--text-primary)]">
                  {t('security.all_secure_msg')}
                </span>
                <span className="text-[11px] text-[var(--text-tertiary)]">
                  {t('security.no_issues')}
                </span>
              </div>
            )}
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
};

export interface DisplaySecurityIssue {
  id: string;
  issue_type: string;
  severity: string;
  entry_id: string;
  entry_title: string;
  description: string;
  is_group?: boolean;
  group_entries?: { id: string; title: string }[];
}

const StatCard: React.FC<{
  icon: React.ReactNode;
  label: string;
  count: number;
  iconClass?: string;
}> = ({ icon, label, count, iconClass = 'bg-[var(--bg-base)] text-[var(--text-secondary)]' }) => (
  <div className="flex items-center gap-2.5 rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] px-3 py-2">
    <div className={`flex h-6 w-6 shrink-0 items-center justify-center rounded-[3px] ${iconClass}`}>
      {icon}
    </div>
    <div className="flex flex-col min-w-0">
      <span className="text-[13px] font-semibold font-mono text-[var(--text-primary)] leading-tight">{count}</span>
      <span className="text-[10.5px] text-[var(--text-tertiary)] truncate mt-0.5">{label}</span>
    </div>
  </div>
);

export function getLocalizedIssueDescription(
  issue: SecurityIssue | DisplaySecurityIssue,
  t: (key: string, params?: Record<string, string | number>) => string
): string {
  switch (issue.issue_type) {
    case 'Breached': {
      const match = issue.description.match(/\d+/);
      const count = match ? match[0] : '1';
      return t('security.desc_breached', { count });
    }
    case 'WeakPassword': {
      return t('security.desc_weak');
    }
    case 'ReusedPassword': {
      if ('is_group' in issue && issue.is_group && issue.group_entries) {
        const services = issue.group_entries.map((e: { title: string }) => e.title).join(', ');
        return t('security.reused_group_desc', { count: issue.group_entries.length, services });
      }
      const parts = issue.description.split(': ');
      const services = parts.length > 1 ? parts[1] : '';
      return services ? t('security.desc_reused_with', { services }) : t('security.desc_reused');
    }
    case 'OldPassword': {
      const match = issue.description.match(/\d+/);
      const days = match ? match[0] : '90';
      return t('security.desc_old_days', { days });
    }
    case 'Missing2FA': {
      return t('security.desc_missing_2fa');
    }
    default:
      return issue.description;
  }
}

const IssueRow: React.FC<{
  issue: DisplaySecurityIssue;
  onClick?: () => void;
  onNavigateToEntry?: (entryId: string) => void;
}> = ({
  issue,
  onClick,
  onNavigateToEntry,
}) => {
  const { t } = useTranslation();
  const description = getLocalizedIssueDescription(issue, t);

  let dotColor = 'bg-[var(--text-tertiary)]';
  if (issue.issue_type === 'Breached') {
    dotColor = 'bg-[var(--text-primary)]';
  } else if (issue.issue_type === 'WeakPassword') {
    dotColor = 'bg-[var(--text-secondary)]';
  } else if (issue.issue_type === 'ReusedPassword') {
    dotColor = 'bg-[var(--text-secondary)]';
  } else if (issue.issue_type === 'OldPassword') {
    dotColor = 'bg-[var(--text-tertiary)]';
  }

  const tooltipTitle = issue.is_group && issue.group_entries
    ? issue.group_entries.map((e) => e.title).join(', ')
    : issue.entry_title;

  return (
    <ActionTooltip
      content={
        <div className="flex flex-col gap-0.5 max-w-xs text-left py-0.5">
          <span className="font-medium text-white">{tooltipTitle}</span>
          <span className="text-[11px] text-zinc-300 leading-snug">{description}</span>
          <span className="text-[10px] text-[var(--text-primary)] mt-0.5 font-medium">
            {t('security.click_to_view_entry', { entry: tooltipTitle })}
          </span>
        </div>
      }
    >
      <div
        onClick={onClick}
        className="group flex items-center gap-2.5 px-3 py-2 text-left transition-colors hover:bg-[var(--bg-hover)] cursor-pointer w-full"
      >
        <span className={`h-2 w-2 rounded-full shrink-0 ${dotColor}`} />
        <div className="flex flex-col gap-0.5 min-w-0 flex-1">
          <span className="text-[12px] font-medium text-[var(--text-primary)] truncate">
            {issue.entry_title}
          </span>
          <span className="text-[11px] text-[var(--text-secondary)] truncate">
            {description}
          </span>
        </div>

        {issue.is_group && issue.group_entries && issue.group_entries.length > 1 ? (
          <div className="flex items-center gap-1.5 shrink-0 ml-auto" onClick={(e) => e.stopPropagation()}>
            {issue.group_entries.map((entry) => (
              <button
                key={entry.id}
                type="button"
                onClick={() => onNavigateToEntry?.(entry.id)}
                className="flex items-center gap-1 h-6 px-2 rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] text-[11px] font-medium text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:border-[var(--text-secondary)] transition-colors cursor-pointer"
              >
                <span>{entry.title}</span>
                <ChevronRight size={11} />
              </button>
            ))}
          </div>
        ) : (
          <div className="flex items-center gap-1.5 shrink-0 ml-auto text-[var(--text-tertiary)] group-hover:text-[var(--text-primary)]">
            <span className="text-[10.5px] font-medium opacity-0 group-hover:opacity-100 transition-opacity">
              {t('security.fix_issue')}
            </span>
            <ChevronRight size={13} className="transition-transform group-hover:translate-x-0.5" />
          </div>
        )}
      </div>
    </ActionTooltip>
  );
};


export default SecurityDashboard;


