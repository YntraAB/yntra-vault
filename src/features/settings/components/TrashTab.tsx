import { useState, useEffect, useCallback } from 'react';
import { Trash, Trash2, RotateCcw, Archive } from 'lucide-react';
import { useAuth } from '@/contexts/AuthContext';
import { useEntries } from '@/contexts/EntriesContext';
import { useToast } from '@/contexts/ToastContext';
import { useTranslation } from '@/contexts/LanguageContext';
import { useBackend } from '@/lib/useBackend';
import { formatBytes } from '@/lib/utils';
import { ActionTooltip } from '@/components/ui/tooltip';
import { SettingSection } from './SettingSection';
import { DeleteTrashModal } from './DeleteTrashModal';
import type { TrashedEntryPreview, VaultStorageMetrics } from '@/lib/backend';

export function TrashTab() {
  const { currentVault } = useAuth();
  const { refreshEntries } = useEntries();
  const { addToast } = useToast();
  const { t } = useTranslation();
  const { backend } = useBackend();

  const [trashItems, setTrashItems] = useState<TrashedEntryPreview[]>([]);
  const [loadingTrash, setLoadingTrash] = useState(false);
  const [metrics, setMetrics] = useState<VaultStorageMetrics | null>(null);
  const [compacting, setCompacting] = useState(false);

  const [deleteModalState, setDeleteModalState] = useState<{
    isOpen: boolean;
    mode: 'single' | 'empty';
    itemId?: string;
    itemTitle?: string;
    itemCount?: number;
  }>({
    isOpen: false,
    mode: 'single',
  });

  const fetchTrash = useCallback(async () => {
    if (!backend || !currentVault) return;
    setLoadingTrash(true);
    try {
      const items = await backend.listTrash();
      setTrashItems(items);
    } catch (e) {
      console.error('Failed to fetch trash previews:', e);
    } finally {
      setLoadingTrash(false);
    }
  }, [backend, currentVault]);

  const fetchMetrics = useCallback(async () => {
    if (!backend || !currentVault) return;
    try {
      const m = await backend.getStorageMetrics();
      setMetrics(m);
    } catch (e) {
      console.error('Failed to fetch storage metrics:', e);
    }
  }, [backend, currentVault]);

  useEffect(() => {
    fetchTrash();
    fetchMetrics();
  }, [fetchTrash, fetchMetrics]);

  const handleCompact = async () => {
    if (!backend) return;
    setCompacting(true);
    try {
      const freshMetrics = await backend.compactVault();
      setMetrics(freshMetrics);
      addToast({ message: t('toast.vault_compacted'), type: 'success' });
      await fetchTrash();
      await refreshEntries();
    } catch (e) {
      addToast({ message: String(e), type: 'error' });
    } finally {
      setCompacting(false);
    }
  };

  const handleRestore = async (id: string) => {
    if (!backend) return;
    try {
      await backend.restoreFromTrash(id);
      addToast({ message: t('settings.entry_restored'), type: 'success' });
      await fetchTrash();
      await fetchMetrics();
      await refreshEntries();
    } catch (e) {
      addToast({ message: t('toast.restore_failed', { err: String(e) }), type: 'error' });
    }
  };

  const promptPermanentDelete = (item: TrashedEntryPreview) => {
    setDeleteModalState({
      isOpen: true,
      mode: 'single',
      itemId: item.id,
      itemTitle: item.title,
    });
  };

  const promptEmptyTrash = () => {
    setDeleteModalState({
      isOpen: true,
      mode: 'empty',
      itemCount: trashItems.length,
    });
  };

  const handleConfirmDelete = async () => {
    if (!backend) return;
    if (deleteModalState.mode === 'empty') {
      try {
        await backend.emptyTrash();
        addToast({ message: t('settings.trash_emptied'), type: 'success' });
        await fetchTrash();
        await fetchMetrics();
      } catch (e) {
        addToast({ message: t('toast.empty_trash_failed', { err: String(e) }), type: 'error' });
      }
    } else if (deleteModalState.itemId) {
      try {
        await backend.permanentDelete(deleteModalState.itemId);
        addToast({ message: t('settings.entry_deleted_permanently'), type: 'success' });
        await fetchTrash();
        await fetchMetrics();
      } catch (e) {
        addToast({ message: t('toast.permanent_delete_failed', { err: String(e) }), type: 'error' });
      }
    }
  };

  return (
    <div className="flex flex-col gap-6">
      <SettingSection label={t('settings.trash_title')}>
        <p className="mb-3 text-[12px] text-[var(--text-secondary)]">
          {t('settings.trash_desc')}
        </p>
        {trashItems.length > 0 && (
          <ActionTooltip content={t('settings.trash_empty')}>
            <button
              onClick={promptEmptyTrash}
              className="mb-4 flex items-center gap-1.5 rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] px-3 py-1.5 text-[12px] font-medium text-[var(--text-secondary)] transition-colors hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)] cursor-pointer"
            >
              <Trash size={13} />
              {t('settings.trash_empty')}
            </button>
          </ActionTooltip>
        )}

        {loadingTrash ? (
          <p className="text-[12px] text-[var(--text-tertiary)] py-4">{t('settings.loading_trash')}</p>
        ) : trashItems.length === 0 ? (
          <div className="flex flex-col items-center justify-center gap-2 py-8 rounded-[3px] border border-dashed border-[var(--border-subtle)]">
            <Trash2 size={20} className="text-[var(--text-tertiary)]" />
            <p className="text-[12px] text-[var(--text-tertiary)]">{t('settings.trash_is_empty')}</p>
          </div>
        ) : (
          <div className="flex flex-col gap-[2px] rounded-[3px] border border-[var(--border-subtle)] overflow-hidden">
            {trashItems.map((item) => (
              <div
                key={item.id}
                className="flex items-center justify-between bg-[var(--bg-elevated)] p-3 text-left transition-colors hover:bg-[var(--bg-hover)]"
              >
                <div className="flex flex-col gap-1 min-w-0">
                  <span title={item.title} className="truncate text-[13px] font-medium text-[var(--text-primary)] select-text">
                    {item.title}
                  </span>
                  <span className="text-[10px] text-[var(--text-secondary)]">
                    {t('settings.trash_deleted_meta', {
                      date: new Date(item.deleted_at).toLocaleDateString(),
                      days: item.days_until_permanent,
                    })}
                  </span>
                </div>
                <div className="flex items-center gap-1 shrink-0">
                  <ActionTooltip content={t('settings.restore_entry')}>
                    <button
                      type="button"
                      onClick={() => handleRestore(item.id)}
                      className="inline-flex h-7 items-center gap-1 rounded-[3px] px-2 text-[11px] font-medium text-[var(--text-secondary)] transition-colors hover:bg-[var(--bg-active)] hover:text-[var(--text-primary)] cursor-pointer"
                    >
                      <RotateCcw size={12} />
                      {t('settings.restore_entry')}
                    </button>
                  </ActionTooltip>
                  <ActionTooltip content={t('settings.delete_permanently')}>
                    <button
                      type="button"
                      onClick={() => promptPermanentDelete(item)}
                      className="inline-flex h-7 items-center gap-1 rounded-[3px] px-2 text-[11px] font-medium text-[var(--text-secondary)] transition-colors hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)] cursor-pointer"
                    >
                      <Trash2 size={12} />
                      {t('settings.delete_permanently')}
                    </button>
                  </ActionTooltip>
                </div>
              </div>
            ))}
          </div>
        )}
      </SettingSection>

      {/* Storage Footprint & Compaction */}
      <SettingSection
        label={t('settings.storage_compaction_title')}
        tooltip={t('settings.storage_compaction_tooltip')}
      >
        <div className="flex flex-col gap-3 rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] p-3">
          <div className="grid grid-cols-3 gap-2">
            <div className="flex flex-col gap-0.5 rounded-[3px] bg-[var(--bg-base)] p-2 border border-[var(--border-subtle)]">
              <span className="text-[10px] font-bold uppercase tracking-wider text-[var(--text-tertiary)]">
                {t('settings.storage_db_file')}
              </span>
              <span className="text-[13px] font-semibold text-[var(--text-primary)]">
                {metrics ? formatBytes(metrics.vault_file_bytes) : '—'}
              </span>
            </div>
            <div className="flex flex-col gap-0.5 rounded-[3px] bg-[var(--bg-base)] p-2 border border-[var(--border-subtle)]">
              <span className="text-[10px] font-bold uppercase tracking-wider text-[var(--text-tertiary)]">
                {t('settings.storage_active_attachments')}
              </span>
              <span className="text-[13px] font-semibold text-[var(--text-primary)]">
                {metrics ? `${formatBytes(metrics.active_attachment_bytes)} (${metrics.active_attachment_count})` : '—'}
              </span>
            </div>
            <div className="flex flex-col gap-0.5 rounded-[3px] bg-[var(--bg-base)] p-2 border border-[var(--border-subtle)]">
              <span className="text-[10px] font-bold uppercase tracking-wider text-[var(--text-tertiary)]">
                {t('settings.storage_trash')}
              </span>
              <span className="text-[13px] font-semibold text-[var(--text-primary)]">
                {metrics ? t('settings.storage_entries_count', { count: metrics.trashed_entry_count }) : '—'}
              </span>
            </div>
          </div>

          <p className="text-[11px] text-[var(--text-secondary)] leading-relaxed">
            {t('settings.storage_compaction_desc')}
          </p>

          <button
            type="button"
            disabled={compacting}
            onClick={handleCompact}
            className="flex items-center justify-center gap-1.5 self-start rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] px-3 py-1.5 text-[12px] font-medium text-[var(--text-primary)] hover:bg-[var(--bg-hover)] disabled:opacity-50 transition-colors cursor-pointer"
          >
            <Archive size={13} className={compacting ? 'animate-spin' : ''} />
            <span>{compacting ? t('settings.compacting') : t('settings.compact_vault_button')}</span>
          </button>
        </div>
      </SettingSection>

      <DeleteTrashModal
        isOpen={deleteModalState.isOpen}
        mode={deleteModalState.mode}
        itemTitle={deleteModalState.itemTitle}
        itemCount={deleteModalState.itemCount}
        onClose={() => setDeleteModalState((prev) => ({ ...prev, isOpen: false }))}
        onConfirm={handleConfirmDelete}
      />
    </div>
  );
}

export default TrashTab;
