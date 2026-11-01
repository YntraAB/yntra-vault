import { ActionTooltip } from '@/components/ui/tooltip';
import { useTranslation } from '@/contexts/LanguageContext';
import { useSettings } from '@/features/settings';
import type { BreachStatus } from '@/lib/backend';
import { useBackend } from '@/lib/useBackend';
import React,{ useEffect,useState } from 'react';
import { useSecurityAudit } from '../hooks/useSecurityAudit';
import { type BreachIndicatorProps,getStatusConfig } from './BreachIndicator.model';


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
  const [checked, setChecked] = useState<{ password: string; source?: BreachStatus; status: BreachStatus } | null>(null);
  const status: BreachStatus = checked && checked.password === password && checked.source === initialStatus ? checked.status : initialStatus ?? { type: 'Unknown' };

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
    if (!backend || !settings.autoBreachCheck || !password || password.length < 4 || initialStatus && initialStatus.type !== 'Unknown') return;
    let cancelled = false;
    const timer = setTimeout(async () => {
      setChecked({ password, source: initialStatus, status: { type: 'Checking' } });
      try {
        const result = await backend.checkPasswordBreach(password);
        if (cancelled) return;
        const status: BreachStatus = result.is_breached
          ? { type: 'Breached', breach_count: result.breach_count, checked_at: result.checked_at }
          : { type: 'Safe', checked_at: result.checked_at };
        setChecked({ password, source: initialStatus, status }); onStatusChange?.(status);
      } catch (error) { if (!cancelled) setChecked({ password, source: initialStatus, status: { type: 'Error', message: String(error) } }); }
    }, 500);
    return () => { cancelled = true; clearTimeout(timer); };
  }, [backend, password, initialStatus, onStatusChange, settings.autoBreachCheck]);

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


export default BreachIndicator;