import { useState, useEffect, useCallback } from 'react';
import { Trash, Trash2, RotateCcw } from 'lucide-react';
import { useAppState } from '@/contexts/AppStateContext';
import { useTranslation } from '@/contexts/LanguageContext';
import { useBackend } from '@/lib/useBackend';
import { ActionTooltip } from '../ui/tooltip';
import { SettingSection } from './SettingSection';
import type { TrashedEntryPreview } from '@/lib/backend';

export function TrashTab() {
  const { currentVault, refreshEntries, addToast } = useAppState();
  const { t } = useTranslation();
  const { backend } = useBackend();

  const [trashItems, setTrashItems] = useState<TrashedEntryPreview[]>([]);
  const [loadingTrash, setLoadingTrash] = useState(false);

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

  useEffect(() => {
    fetchTrash();
  }, [fetchTrash]);

  const handleRestore = async (id: string) => {
    if (!backend) return;
    try {
      await backend.restoreFromTrash(id);
      addToast({ message: t('settings.entry_restored'), type: 'success' });
      await fetchTrash();
      await refreshEntries();
    } catch (e) {
      addToast({ message: `Restore failed: ${e}`, type: 'error' });
    }
  };

  const handlePermanentDelete = async (id: string) => {
    if (!backend) return;
    if (!confirm(t('settings.confirm_permanent_delete'))) return;
    try {
      await backend.permanentDelete(id);
      addToast({ message: t('settings.entry_deleted_permanently'), type: 'success' });
      await fetchTrash();
    } catch (e) {
      addToast({ message: `Permanent delete failed: ${e}`, type: 'error' });
    }
  };

  const handleEmptyTrash = async () => {
    if (!backend) return;
    if (!confirm(t('settings.confirm_empty_trash'))) return;
    try {
      await backend.emptyTrash();
      addToast({ message: t('settings.trash_emptied'), type: 'success' });
      await fetchTrash();
    } catch (e) {
      addToast({ message: `Empty trash failed: ${e}`, type: 'error' });
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
              onClick={handleEmptyTrash}
              className="mb-4 flex items-center gap-1.5 rounded-[3px] border border-red-500/30 bg-red-500/10 px-3 py-1.5 text-[12px] font-medium text-red-500 transition-colors hover:bg-red-500 hover:text-white"
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
                  <span className="truncate text-[13px] font-medium text-[var(--text-primary)]">
                    {item.title}
                  </span>
                  <span className="text-[10px] text-[var(--text-secondary)]">
                    Deleted: {new Date(item.deleted_at).toLocaleDateString()} • {item.days_until_permanent} days remaining
                  </span>
                </div>
                <div className="flex items-center gap-1 shrink-0">
                  <ActionTooltip content={t('settings.restore_entry')}>
                    <button
                      type="button"
                      onClick={() => handleRestore(item.id)}
                      className="inline-flex h-7 items-center gap-1 rounded-[3px] px-2 text-[11px] font-medium text-[var(--text-secondary)] transition-colors hover:bg-[var(--bg-active)] hover:text-[var(--text-primary)]"
                    >
                      <RotateCcw size={12} />
                      {t('settings.restore_entry')}
                    </button>
                  </ActionTooltip>
                  <ActionTooltip content={t('settings.delete_permanently')}>
                    <button
                      type="button"
                      onClick={() => handlePermanentDelete(item.id)}
                      className="inline-flex h-7 items-center gap-1 rounded-[3px] px-2 text-[11px] font-medium text-[var(--destructive)] transition-colors hover:bg-[var(--destructive)]/10"
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
    </div>
  );
}
