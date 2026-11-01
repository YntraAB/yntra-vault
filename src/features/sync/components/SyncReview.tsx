import { syncFieldKeys as fieldKeys, syncFieldLabel } from '../fieldLabel';
import { useEffect, useRef, useState } from 'react';
import { motion, useReducedMotion } from 'framer-motion';
import { useAuth } from '@/features/auth';
import { useTranslation } from '@/contexts/LanguageContext';
import type { YntraVaultBackend } from '@/lib/backend';
import type { SyncChange, SyncDecision, SyncPreview } from '../types';

const button = 'min-h-[44px] rounded-[3px] border border-[var(--border)] px-3 text-[12px] hover:bg-[var(--bg-surface)] focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 disabled:opacity-50';

export function SyncReviewHost() {
  const { backend, isLocked, currentVault, getSessionGeneration, isSessionCurrent } = useAuth();
  const [pending, setPending] = useState<{ value: SyncPreview | null; generation: number } | null>(null);
  useEffect(() => {
    if (!backend?.onSyncReview || isLocked || !currentVault) return;
    let disposed = false;
    let unsubscribe: (() => void) | undefined;
    const generation = getSessionGeneration();
    const accept = (value: SyncPreview | null) => {
      if (!disposed && isSessionCurrent(generation)) setPending({ value, generation });
    };
    let receivedEvent = false;
    void backend.onSyncReview(value => { receivedEvent = true; accept(value); }).then(async stop => {
      if (disposed) { stop(); return; }
      unsubscribe = stop;
      const value = await backend.getSyncReview();
      if (!receivedEvent) accept(value);
    }).catch(() => {});
    return () => { disposed = true; unsubscribe?.(); };
  }, [backend, isLocked, currentVault, getSessionGeneration, isSessionCurrent]);
  if (!backend || isLocked || !pending?.value || !isSessionCurrent(pending.generation)) return null;
  return <PendingReview key={`${currentVault?.id}:${pending.value.id}`} backend={backend} preview={pending.value} onClose={() => setPending(null)} />;
}

function PendingReview(props: { backend: YntraVaultBackend; preview: SyncPreview; onClose: () => void }) {
  const [open, setOpen] = useState(!props.preview.background);
  const { t } = useTranslation();
  return open ? <SyncReview {...props} /> : <div role="status" className="fixed bottom-20 right-4 z-50 max-w-[calc(100%_-_32px)] rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] p-3 text-[var(--text-primary)] shadow-lg">
    <button type="button" className={button} onClick={() => setOpen(true)}>{t('sync.review_title')}</button>
  </div>;
}

export function SyncReview({ backend, preview, onClose }: { backend: YntraVaultBackend; preview: SyncPreview; onClose: () => void }) {
  const { t } = useTranslation();
  const { getSessionGeneration, isSessionCurrent } = useAuth();
  const dialog = useRef<HTMLDialogElement>(null);
  const [decisions, setDecisions] = useState<Record<string, SyncDecision>>(() => Object.fromEntries(preview.changes.filter(c => c.conflict && c.suggested).map(c => [c.key, { key: c.key, choice: c.suggested! }])));
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const reducedMotion = useReducedMotion();
  const groups = new Map<string, SyncChange[]>();
  for (const change of preview.changes) {
    const fields = groups.get(change.entry_id) ?? [];
    fields.push(change);
    groups.set(change.entry_id, fields);
  }
  useEffect(() => {
    const element = dialog.current;
    const previous = document.activeElement;
    element?.showModal();
    return () => { element?.close(); if (previous instanceof HTMLElement && previous.isConnected) previous.focus(); };
  }, []);
  const duplicated = new Set(preview.changes.filter(c => c.keep_both_entries && decisions[c.key]?.choice === 'both').map(c => c.entry_id));
  const unresolved = preview.changes.filter(change => change.conflict && !decisions[change.key] && !duplicated.has(change.entry_id)).length;
  const choose = (change: SyncChange, decision: SyncDecision) => setDecisions(previous => {
    const next = { ...previous };
    if (change.keep_both_entries && decision.choice === 'both') {
      for (const field of preview.changes) if (field.entry_id === change.entry_id) delete next[field.key];
    }
    next[change.key] = decision;
    return next;
  });
  const cancel = async () => {
    if (busy) return;
    try { await backend.cancelSyncReview(); onClose(); } catch (err) { setError(String(err)); }
  };
  const approve = async () => {
    if (busy || unresolved) return;
    setBusy(true); setError('');
    const generation = getSessionGeneration();
    try {
      await backend.approveSyncReview(preview.id, Object.values(decisions));
      if (isSessionCurrent(generation)) onClose();
    } catch (err) {
      if (isSessionCurrent(generation)) { setError(String(err)); setBusy(false); }
    }
  };
  return <dialog ref={dialog} aria-labelledby="sync-review-title" onCancel={event => { event.preventDefault(); void cancel(); }} className="m-auto max-h-[90dvh] overflow-hidden w-[min(680px,calc(100%_-_24px))] rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] p-0 text-[var(--text-primary)] backdrop:bg-black/50">
    <motion.div initial={reducedMotion ? false : { opacity: 0, y: 6 }} animate={{ opacity: 1, y: 0 }} className="flex max-h-[calc(90dvh_-_2px)] flex-col">
      <header className="shrink-0 border-b border-[var(--border)] p-4">
        <h2 id="sync-review-title" className="text-[15px] font-semibold">{t('sync.review_title')}</h2>
        <p className="mt-1 text-[12px] text-[var(--text-secondary)]">{t('sync.review_description')}</p>
      </header>
      <div className="min-h-0 overflow-y-auto p-4">
        <p role="status" className="mb-3 text-[12px]">{t('sync.review_count', { count: preview.changes.length, conflicts: unresolved })}</p>
        <div className="divide-y divide-[var(--border)]">
          {Array.from(groups, ([id, changes]) => <details key={id} open={changes.some(change => change.conflict)} className="py-2">
            <summary className="min-h-[44px] cursor-pointer break-words py-3 text-[13px] font-medium focus-visible:outline focus-visible:outline-2">
              {changes[0].title || id} <span className="font-normal text-[var(--text-secondary)]">({changes.length})</span>
            </summary>
            {changes.map(change => <ChangeRow key={change.key} change={change} previewId={preview.id} backend={backend} decision={decisions[change.key]} disabled={busy || duplicated.has(change.entry_id) && decisions[change.key]?.choice !== 'both'} onChange={decision => choose(change, decision)} />)}
          </details>)}
        </div>
      </div>
      <footer className="shrink-0 border-t border-[var(--border)] p-4">
        {error && <p role="alert" className="mb-3 text-[12px]">{error}</p>}
        <div className="flex justify-end gap-2">
          <button type="button" className={button} onClick={() => void cancel()} disabled={busy}>{t('common.cancel')}</button>
          <button type="button" className={`${button} bg-[var(--text-primary)] text-[var(--bg-base)]`} onClick={() => void approve()} disabled={busy || unresolved > 0}>{busy ? t('settings.syncing') : t('sync.review_apply')}</button>
        </div>
      </footer>
    </motion.div>
  </dialog>;
}

function ChangeRow({ change, previewId, backend, decision, disabled, onChange }: { change: SyncChange; previewId: string; backend: YntraVaultBackend; decision?: SyncDecision; disabled: boolean; onChange: (decision: SyncDecision) => void }) {
  const { t, language } = useTranslation();
  const { getSessionGeneration, isSessionCurrent } = useAuth();
  const [values, setValues] = useState<{ local: string; remote: string } | null>(null);
  const [error, setError] = useState('');
  const [loading, setLoading] = useState(false);
  const alive = useRef(true);
  useEffect(() => { alive.current = true; return () => { alive.current = false; }; }, []);
  const reveal = async () => {
    if (values) { setValues(null); return; }
    if (loading) return;
    setLoading(true);
    const generation = getSessionGeneration();
    try {
      const [local, remote] = await Promise.all([backend.inspectSyncField(previewId, change.key, false), backend.inspectSyncField(previewId, change.key, true)]);
      if (alive.current && isSessionCurrent(generation)) setValues({ local, remote });
    } catch (err) { if (alive.current && isSessionCurrent(generation)) setError(String(err)); }
    finally { if (alive.current && isSessionCurrent(generation)) setLoading(false); }
  };
  const selected = decision?.choice ?? change.suggested;
  const latest = change.local_changed_at && change.remote_changed_at && change.local_changed_at !== change.remote_changed_at
    ? (Date.parse(change.local_changed_at) > Date.parse(change.remote_changed_at) ? 'local' : 'remote') : null;
  const sides = latest === 'remote' ? ['remote', 'local'] as const : ['local', 'remote'] as const;
  const isFieldOrder = change.custom_field_name === '_field_order';
  const label = syncFieldLabel(change, t);
  const displayedValue = (value: string) => {
    if (!isFieldOrder) return value || t('sync.review_empty');
    try {
      const order = (JSON.parse(value) as { value?: string } | null)?.value;
      if (!order) return t('sync.review_empty');
      return order.split(',').map(field => field === 'password' ? t('detail.password') : fieldKeys[field] ? t(fieldKeys[field]) : t('sync.review_custom')).join(' → ');
    } catch { return t('sync.review_empty'); }
  };
  return <fieldset disabled={disabled} className="min-w-0 py-3">
    <legend className="max-w-full break-words text-[13px] font-medium">{label}</legend>
    <p className="mb-2 text-[11px] text-[var(--text-secondary)]">{t(change.conflict ? 'sync.review_conflict' : 'sync.review_suggestion')}</p>
    <div className="grid gap-2 sm:grid-cols-2">
      {sides.map(side => {
        const time = side === 'local' ? change.local_changed_at : change.remote_changed_at;
        const state = side === 'local' ? change.local_state : change.remote_state;
        const device = side === 'local' ? change.local_device_name : change.remote_device_name;
        return <label key={side} className="flex min-h-[44px] min-w-0 cursor-pointer gap-2 rounded-[3px] border border-[var(--border)] p-2.5 text-[12px]">
          <input type="radio" aria-label={t(side === 'local' ? 'sync.review_local' : 'sync.review_remote')} name={change.key} value={side} checked={selected === side} onChange={() => onChange({ key: change.key, choice: side })} className="mt-1" />
          <span className="min-w-0 flex-1">
            <span className="block font-medium">{t(side === 'local' ? 'sync.review_local' : 'sync.review_remote')}{latest === side ? ` · ${t('sync.review_latest')}` : ''}</span>
            {device && <span className="block break-words text-[11px] text-[var(--text-secondary)]">{device}</span>}
            {state && <span className="block">{t(`sync.state_${state}`)}</span>}
            <span className="block text-[11px] text-[var(--text-secondary)]">{time ? new Date(time).toLocaleString(language) : t('sync.review_unknown_time')}{change.conflict && !change.time_order_reliable && time ? ` · ${t('sync.review_unknown_time')}` : ''}</span>
            <span className="mt-2 block whitespace-pre-wrap break-words [overflow-wrap:anywhere]">{values ? displayedValue(values[side]) : '••••••••'}</span>
          </span>
        </label>;
      })}
    </div>
    <div className="mt-2 flex flex-wrap items-center gap-2">
      <button type="button" className={button} disabled={loading} onClick={() => void reveal()}>{t(values ? 'sync.review_hide' : 'sync.review_show')}</button>
      {change.can_keep_both && <label className="flex min-h-[44px] items-center gap-2 text-[12px]"><input type="radio" aria-label={t('sync.review_both')} name={change.key} checked={selected === 'both'} onChange={() => onChange({ key: change.key, choice: 'both' })} />{t('sync.review_both')}</label>}
      {change.can_edit && <label className="flex min-h-[44px] items-center gap-2 text-[12px]"><input type="radio" aria-label={t('sync.review_edit')} name={change.key} checked={selected === 'edited'} onChange={() => onChange({ key: change.key, choice: 'edited', text: '' })} />{t('sync.review_edit')}</label>}
    </div>
    {selected === 'both' && <p className="mt-1 text-[12px] text-[var(--text-secondary)]">{t(change.field === 'notes' ? 'sync.review_both_notes' : 'sync.review_both_passwords')}</p>}
    {selected === 'both' && change.keep_both_entries && <ul className="mt-2 list-disc space-y-1 pl-5 text-[12px]">{change.keep_both_entries.map(item => <li key={item.id}>{item.title}</li>)}</ul>}
    {selected === 'edited' && <textarea aria-label={t('sync.review_edit')} value={decision?.text ?? ''} onChange={event => onChange({ key: change.key, choice: 'edited', text: event.target.value })} className="mt-2 min-h-24 w-full rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] p-2 text-[12px]" />}
    {error && <p role="alert" className="mt-2 text-[12px]">{error}</p>}
  </fieldset>;
}
