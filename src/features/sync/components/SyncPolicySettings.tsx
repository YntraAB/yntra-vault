import { useEffect, useState } from 'react';
import { useAuth } from '@/features/auth';
import { useTranslation } from '@/contexts/LanguageContext';
import { SettingSection, Toggle } from '@/features/settings/components/SettingSection';
import type { SyncPolicy } from '../types';

export function SyncPolicySettings() {
  const { backend, currentVault, getSessionGeneration, isSessionCurrent } = useAuth();
  const { t } = useTranslation();
  const [policy, setPolicy] = useState<SyncPolicy | null>(null);
  const [error, setError] = useState('');
  const [saving, setSaving] = useState(false);
  useEffect(() => {
    if (!backend?.getSyncPolicy) return;
    let active = true;
    const generation = getSessionGeneration();
    void backend.getSyncPolicy().then(value => { if (active && isSessionCurrent(generation)) setPolicy(value); }).catch(err => { if (active) setError(String(err)); });
    return () => { active = false; };
  }, [backend, currentVault, getSessionGeneration, isSessionCurrent]);
  const update = async (next: SyncPolicy) => {
    if (!backend || saving) return;
    const generation = getSessionGeneration();
    setSaving(true); setError('');
    try { await backend.setSyncPolicy(next); if (isSessionCurrent(generation)) setPolicy(next); }
    catch (err) { if (isSessionCurrent(generation)) setError(String(err)); }
    finally { if (isSessionCurrent(generation)) setSaving(false); }
  };
  return <SettingSection label={t('sync.policy_title')}>
    <p className="mb-3 text-[12px] text-[var(--text-secondary)]">{t('sync.policy_description')}</p>
    <fieldset disabled={!policy || saving} className="space-y-3">
      <label className="flex min-h-[44px] items-center justify-between gap-4 text-[12px]">{t('sync.policy_conflicts_only')}<Toggle label={t('sync.policy_conflicts_only')} checked={policy?.review_conflicts_only ?? false} onChange={value => { if (policy) void update({ ...policy, review_conflicts_only: value }); }} /></label>
      <label className="flex min-h-[44px] items-center justify-between gap-4 text-[12px]">{t('sync.policy_latest')}<Toggle label={t('sync.policy_latest')} checked={policy?.prefer_latest ?? false} onChange={value => { if (policy) void update({ ...policy, prefer_latest: value }); }} /></label>
    </fieldset>
    {error && <p role="alert" className="mt-2 text-[12px]">{error}</p>}
  </SettingSection>;
}
