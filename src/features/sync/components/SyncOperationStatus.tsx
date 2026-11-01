import { useEffect, useState } from 'react';
import { useAuth } from '@/features/auth';
import { useTranslation } from '@/contexts/LanguageContext';
import type { SyncOperation } from '../types';
import { updateSyncOperations } from '../operations';

export function SyncOperationStatus() {
  const { backend, currentVault, isLocked, getSessionGeneration, isSessionCurrent } = useAuth();
  const { t } = useTranslation();
  const [snapshot, setSnapshot] = useState<{ generation: number; operations: SyncOperation[] } | null>(null);
  const [dismissed, setDismissed] = useState<string[]>([]);
  useEffect(() => {
    if (!backend?.onSyncOperation || isLocked || !currentVault) return;
    const generation = getSessionGeneration();
    let disposed = false;
    let eventReceived = false;
    let unsubscribe: (() => void) | undefined;
    void backend.onSyncOperation(operation => {
      eventReceived = true;
      if (!disposed && isSessionCurrent(generation)) {
        setSnapshot(previous => ({ generation, operations: updateSyncOperations(previous?.generation === generation ? previous.operations : [], operation) }));
      }
    }).then(async stop => {
      if (disposed) { stop(); return; }
      unsubscribe = stop;
      const operations = await backend.getSyncOperations();
      if (!disposed && !eventReceived && isSessionCurrent(generation)) setSnapshot({ generation, operations });
    }).catch(() => {});
    return () => { disposed = true; unsubscribe?.(); };
  }, [backend, currentVault, isLocked, getSessionGeneration, isSessionCurrent]);
  if (isLocked || !snapshot || !isSessionCurrent(snapshot.generation)) return null;
  const active = snapshot.operations.filter(o => !dismissed.includes(o.id) && !['connecting', 'listening', 'reviewing', 'complete', 'cancelled'].includes(o.phase));
  if (!active.length) return null;
  const operation = active[active.length - 1];
  const failed = operation.phase === 'failed' || operation.phase === 'incomplete';
  return <aside role={failed ? 'alert' : 'status'} className="fixed bottom-[calc(5rem_+_env(safe-area-inset-bottom))] right-4 z-50 max-w-[min(420px,calc(100%_-_32px))] rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] p-3 text-[12px] text-[var(--text-primary)] shadow-lg md:bottom-4">
    <p>{t(`sync.phase_${operation.phase}`)}</p>
    {operation.error && <p className="mt-1 break-words text-[var(--text-secondary)]">{operation.error}</p>}
    {failed && <button type="button" className="mt-2 min-h-[44px] px-2 underline" onClick={() => setDismissed(previous => [...previous, operation.id])}>{t('common.close')}</button>}
  </aside>;
}
