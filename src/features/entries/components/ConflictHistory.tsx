import { useEffect, useRef, useState } from 'react';
import { History } from 'lucide-react';
import type { ConflictHistoryItem, YntraVaultBackend } from '@/lib/backend';
import type { SyncPreview } from '@/features/sync/types';
import { syncFieldLabel } from '@/features/sync/fieldLabel';
import { useAuth } from '@/features/auth/context/AuthContext';
import { useEntries } from '../context/EntriesContext';
import { useTranslation } from '@/contexts/LanguageContext';

const control = 'min-h-[44px] rounded-[3px] border border-[var(--border)] px-3 py-2 text-[12px] hover:bg-[var(--bg-hover)] focus-visible:outline focus-visible:outline-2 disabled:opacity-50';

export function ConflictHistoryButton({ entryId, backend }: { entryId: string; backend: YntraVaultBackend }) {
  const [open, setOpen] = useState(false);
  const { t } = useTranslation();
  const { refreshEntries } = useEntries();
  return <>
    <button type="button" aria-label={t('history.conflicts')} title={t('history.conflicts')} className="inline-flex h-11 w-11 shrink-0 items-center justify-center rounded-[3px] text-[var(--text-secondary)] hover:bg-[var(--bg-hover)]" onClick={() => setOpen(true)}><History size={16} /></button>
    {open && <ConflictHistoryDialog entryId={entryId} backend={backend} onClose={() => setOpen(false)} onRestored={refreshEntries} />}
  </>;
}

export function ConflictHistoryDialog({ entryId, backend, onClose, onRestored }: { entryId: string; backend: YntraVaultBackend; onClose: () => void; onRestored: () => Promise<void> }) {
  const { t } = useTranslation();
  const { getSessionGeneration, isSessionCurrent } = useAuth();
  const dialog = useRef<HTMLDialogElement>(null);
  const alive = useRef(false);
  const request = useRef(0);
  const [versions, setVersions] = useState<ConflictHistoryItem[]>([]);
  const [selected, setSelected] = useState<string | null>(null);
  const [preview, setPreview] = useState<SyncPreview | null>(null);
  const [values, setValues] = useState<Record<string, string>>({});
  const [busy, setBusy] = useState(true);
  const [error, setError] = useState('');
  useEffect(() => {
    alive.current = true;
    const token = ++request.current;
    const generation = getSessionGeneration();
    const element = dialog.current;
    const previous = document.activeElement;
    element?.showModal();
    void backend.listConflictHistory(entryId).then(result => {
      if (alive.current && token === request.current && isSessionCurrent(generation)) setVersions(result);
    }).catch(reason => { if (alive.current && token === request.current && isSessionCurrent(generation)) setError(String(reason)); })
      .finally(() => { if (alive.current && token === request.current && isSessionCurrent(generation)) setBusy(false); });
    return () => { alive.current = false; element?.close(); if (previous instanceof HTMLElement && previous.isConnected) previous.focus(); };
  }, [backend, entryId, getSessionGeneration, isSessionCurrent]);
  const select = async (versionId: string) => {
    const generation = getSessionGeneration(), token = ++request.current;
    setSelected(versionId); setPreview(null); setValues({}); setBusy(true); setError('');
    try { const result = await backend.previewConflictHistory(entryId, versionId); if (alive.current && token === request.current && isSessionCurrent(generation)) setPreview(result); }
    catch (reason) { if (alive.current && token === request.current && isSessionCurrent(generation)) setError(String(reason)); }
    finally { if (alive.current && token === request.current && isSessionCurrent(generation)) setBusy(false); }
  };
  const reveal = async (field: string) => {
    if (!selected) return;
    if (Object.hasOwn(values, field)) { setValues(previous => { const next = { ...previous }; delete next[field]; return next; }); return; }
    const generation = getSessionGeneration(), token = request.current;
    try { const value = await backend.inspectConflictHistory(entryId, selected, field); if (alive.current && token === request.current && isSessionCurrent(generation)) setValues(previous => ({ ...previous, [field]: value })); }
    catch (reason) { if (alive.current && token === request.current && isSessionCurrent(generation)) setError(String(reason)); }
  };
  const restore = async () => {
    if (!selected || !preview || busy) return;
    const generation = getSessionGeneration();
    setBusy(true); setError('');
    try {
      await backend.restoreConflictHistory(entryId, selected, preview.local_revision);
      if (!alive.current || !isSessionCurrent(generation)) return;
      await onRestored();
      if (alive.current && isSessionCurrent(generation)) onClose();
    } catch (reason) { if (alive.current && isSessionCurrent(generation)) setError(String(reason)); }
    finally { if (alive.current && isSessionCurrent(generation)) setBusy(false); }
  };
  return <dialog ref={dialog} aria-labelledby="conflict-history-title" onCancel={event => { event.preventDefault(); onClose(); }} className="m-auto w-[min(640px,calc(100%_-_24px))] max-h-[85dvh] overflow-y-auto rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] p-4 text-[var(--text-primary)] backdrop:bg-black/50">
    <h2 id="conflict-history-title" className="text-[16px] font-medium">{t('history.conflicts')}</h2>
    <p className="my-3 text-[12px] text-[var(--text-secondary)]">{t('history.description')}</p>
    {busy && <p role="status">{t('common.loading')}</p>}
    {error && <p role="alert" className="my-2 text-[var(--destructive)]">{error}</p>}
    {!busy && !versions.length && !error && <p>{t('history.empty')}</p>}
    <div className="flex flex-wrap gap-2">{versions.map((version, index) => <button type="button" key={version.id} className={control} aria-pressed={selected === version.id} disabled={busy} onClick={() => void select(version.id)}>{new Date(version.saved_at).toLocaleString()} · {index + 1}</button>)}</div>
    {preview && <div className="my-4 space-y-3">
      <h3 className="text-[13px] font-medium">{t('history.changed_fields')}</h3>
      {!preview.changes.length && <p>{t('history.identical')}</p>}
      {preview.changes.map(change => <div key={change.key} className="border-b border-[var(--border)] pb-3">
        <div className="flex items-center justify-between gap-2"><span className="text-[13px]">{syncFieldLabel(change, t)}</span><button type="button" className={control} onClick={() => void reveal(change.field)}>{t(Object.hasOwn(values, change.field) ? 'sync.review_hide' : 'sync.review_show')}</button></div>
        <pre className="mt-2 whitespace-pre-wrap break-words text-[12px]">{Object.hasOwn(values, change.field) ? values[change.field] || t('sync.review_empty') : '••••••••'}</pre>
      </div>)}
    </div>}
    <div className="mt-4 flex flex-wrap justify-end gap-2"><button type="button" className={control} onClick={onClose}>{t('common.close')}</button><button type="button" className={control} disabled={busy || !preview?.changes.length} onClick={() => void restore()}>{t('history.restore')}</button></div>
  </dialog>;
}
