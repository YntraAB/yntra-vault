/**
 * SecurityDashboard — Overview of vault security health
 * 
 * Shows health score, issue counts, and actionable items.
 * Accessible from Settings panel.
 */

import React, { useEffect } from 'react';
import { useSecurityAudit } from '../lib/useBackend';
import type { SecurityIssue, IssueSeverity } from '../lib/backend';
import { useAppState } from '../contexts/AppStateContext';
import {
  ShieldAlert, ShieldCheck, AlertTriangle,
  Key, Clock, Copy, Lock, RefreshCw,
} from 'lucide-react';
import { Skeleton } from './ui/skeleton';
import { motion, AnimatePresence } from 'framer-motion';

interface SecurityDashboardProps {
  onNavigateToEntry?: (entryId: string) => void;
}

export const SecurityDashboard: React.FC<SecurityDashboardProps> = ({
  onNavigateToEntry,
}) => {
  const { audit, loading, runAudit } = useSecurityAudit();
  const { settings } = useAppState();
  const disableDelays = settings.disableSkeletonDelays;

  useEffect(() => {
    runAudit(disableDelays);
  }, [runAudit, disableDelays]);

  const scoreColor = audit
    ? (audit.health_score >= 80 ? '#22c55e' : audit.health_score >= 50 ? '#f59e0b' : '#ef4444')
    : '#6b7280';

  return (
    <div className="flex flex-col gap-5 p-4">
      <AnimatePresence mode="wait">
        {loading && !audit ? (
          <motion.div
            key="audit-skeleton"
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            transition={{ duration: 0.1, ease: 'easeInOut' }}
            className="flex flex-col gap-5"
          >
            {/* Health Score */}
            <div className="flex items-center gap-4 rounded-xl border border-[var(--border)] bg-[var(--bg-surface)] p-4">
              <Skeleton className="h-16 w-16 rounded-full shrink-0" />
              <div className="flex flex-col gap-1.5">
                <Skeleton className="h-4 w-28 rounded" />
                <Skeleton className="h-3 w-20 rounded" />
              </div>
            </div>

            {/* Issue Summary Cards */}
            <div className="grid grid-cols-2 gap-2">
              {[...Array(6)].map((_, i) => (
                <div key={i} className="flex items-center gap-2.5 rounded-lg border border-[var(--border)] bg-[var(--bg-surface)] px-3 py-2.5">
                  <Skeleton className="h-7 w-7 rounded-md shrink-0" />
                  <div className="flex flex-col gap-1.5 flex-1 min-w-0">
                    <Skeleton className="h-4 w-6 rounded" />
                    <Skeleton className="h-2.5 w-14 rounded" />
                  </div>
                </div>
              ))}
            </div>

            {/* Issues List */}
            <div className="flex flex-col gap-2">
              <Skeleton className="h-4 w-20 mb-1 rounded" />
              {[...Array(3)].map((_, i) => (
                <div key={i} className="flex items-center gap-2.5 rounded-md px-2.5 py-2">
                  <Skeleton className="h-2 w-2 rounded-full shrink-0" />
                  <div className="flex-1 flex flex-col gap-1.5 min-w-0">
                    <Skeleton className="h-3.5 w-24 rounded" />
                    <Skeleton className="h-2.5 w-40 rounded" />
                  </div>
                  <Skeleton className="h-3 w-3 shrink-0 rounded" />
                </div>
              ))}
            </div>
          </motion.div>
        ) : !audit ? null : (
          <motion.div
            key="audit-content"
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            transition={{ duration: 0.1, ease: 'easeInOut' }}
            className="flex flex-col gap-5"
          >
            {/* Health Score */}
            <div className="flex items-center gap-4 rounded-xl border border-[var(--border)] bg-[var(--bg-surface)] p-4">
              <div className="relative flex h-16 w-16 items-center justify-center">
                <svg viewBox="0 0 36 36" className="h-16 w-16 -rotate-90">
                  <circle
                    cx="18" cy="18" r="16"
                    fill="none"
                    stroke="var(--bg-elevated)"
                    strokeWidth="3"
                  />
                  <circle
                    cx="18" cy="18" r="16"
                    fill="none"
                    stroke={scoreColor}
                    strokeWidth="3"
                    strokeDasharray={`${audit.health_score} 100`}
                    strokeLinecap="round"
                    className="transition-all duration-700"
                  />
                </svg>
                <span
                  className="absolute text-[18px] font-bold"
                  style={{ color: scoreColor }}
                >
                  {audit.health_score}
                </span>
              </div>

              <div className="flex flex-col gap-1">
                <h3 className="text-[15px] font-semibold text-[var(--text-primary)]">
                  Security Score
                </h3>
                <p className="text-[12px] text-[var(--text-secondary)]">
                  {audit.total_entries} passwords analyzed
                </p>
              </div>

              <button
                onClick={() => runAudit(disableDelays)}
                className="ml-auto rounded-lg p-2 transition-colors hover:bg-[var(--bg-elevated)]"
                title="Refresh audit"
              >
                <RefreshCw size={16} className={`text-[var(--text-tertiary)] ${loading ? 'animate-spin' : ''}`} />
              </button>
            </div>

            {/* Issue Summary Cards */}
            <div className="grid grid-cols-2 gap-2">
              <StatCard
                icon={<ShieldAlert size={16} />}
                label="Breached"
                count={audit.breached_count}
                color="#ef4444"
              />
              <StatCard
                icon={<Key size={16} />}
                label="Weak"
                count={audit.weak_count}
                color="#f59e0b"
              />
              <StatCard
                icon={<Copy size={16} />}
                label="Reused"
                count={audit.reused_count}
                color="#8b5cf6"
              />
              <StatCard
                icon={<Clock size={16} />}
                label="Old (90+ days)"
                count={audit.old_count}
                color="#6b7280"
              />
              <StatCard
                icon={<Lock size={16} />}
                label="Missing 2FA"
                count={audit.no_2fa_count}
                color="#3b82f6"
              />
              <StatCard
                icon={<ShieldCheck size={16} />}
                label="Secure"
                count={audit.total_entries - audit.breached_count - audit.weak_count}
                color="#22c55e"
              />
            </div>

            {/* Issues List */}
            {audit.issues.length > 0 && (
              <div className="flex flex-col gap-1">
                <h4 className="text-[13px] font-semibold text-[var(--text-primary)] mb-1">
                  Issues ({audit.issues.length})
                </h4>
                {audit.issues.map((issue, i) => (
                  <IssueRow
                    key={`${issue.entry_id}-${i}`}
                    issue={issue}
                    onClick={() => onNavigateToEntry?.(issue.entry_id)}
                  />
                ))}
              </div>
            )}

            {audit.issues.length === 0 && (
              <div className="flex flex-col items-center gap-2 rounded-lg border border-[var(--border)] bg-[var(--bg-surface)] p-6">
                <ShieldCheck size={32} className="text-green-500" />
                <span className="text-[13px] font-medium text-[var(--text-primary)]">
                  All passwords are secure!
                </span>
                <span className="text-[11px] text-[var(--text-tertiary)]">
                  No issues found in your vault.
                </span>
              </div>
            )}
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
};

// ─── Stat Card ──────────────────────────────────────────────────────────

const StatCard: React.FC<{
  icon: React.ReactNode;
  label: string;
  count: number;
  color: string;
}> = ({ icon, label, count, color }) => (
  <div className="flex items-center gap-2.5 rounded-lg border border-[var(--border)] bg-[var(--bg-surface)] px-3 py-2.5">
    <div
      className="flex h-7 w-7 items-center justify-center rounded-md"
      style={{ backgroundColor: `${color}15`, color }}
    >
      {icon}
    </div>
    <div className="flex flex-col">
      <span className="text-[15px] font-semibold text-[var(--text-primary)]">{count}</span>
      <span className="text-[10px] text-[var(--text-tertiary)]">{label}</span>
    </div>
  </div>
);

// ─── Issue Row ──────────────────────────────────────────────────────────

const SEVERITY_COLORS: Record<IssueSeverity, string> = {
  Critical: '#ef4444',
  Warning: '#f59e0b',
  Info: '#6b7280',
};

const IssueRow: React.FC<{ issue: SecurityIssue; onClick?: () => void }> = ({
  issue,
  onClick,
}) => (
  <button
    onClick={onClick}
    className="flex items-center gap-2.5 rounded-md px-2.5 py-2 text-left transition-colors hover:bg-[var(--bg-elevated)]"
  >
    <div
      className="h-1.5 w-1.5 rounded-full shrink-0"
      style={{ backgroundColor: SEVERITY_COLORS[issue.severity] }}
    />
    <div className="flex flex-col gap-0.5 min-w-0">
      <span className="text-[12px] font-medium text-[var(--text-primary)] truncate">
        {issue.entry_title}
      </span>
      <span className="text-[11px] text-[var(--text-tertiary)] truncate">
        {issue.description}
      </span>
    </div>
    <AlertTriangle
      size={12}
      className="ml-auto shrink-0"
      style={{ color: SEVERITY_COLORS[issue.severity] }}
    />
  </button>
);

export default SecurityDashboard;



