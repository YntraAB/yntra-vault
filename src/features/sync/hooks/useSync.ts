import { useState, useCallback } from 'react';
import { useBackend } from '@/lib/useBackend';
import { useAuth } from '@/features/auth';
import { useToast } from '@/contexts/ToastContext';
import { useTranslation } from '@/contexts/LanguageContext';
import { saveFileDialog } from '@/lib/backend';

export function useSync() {
  const { backend } = useBackend();
  const { currentVault } = useAuth();
  const { addToast } = useToast();
  const { t } = useTranslation();

  const [isTestingWebdav, setIsTestingWebdav] = useState(false);
  const [isSyncingWebdav, setIsSyncingWebdav] = useState(false);
  const [isSyncingP2P, setIsSyncingP2P] = useState(false);

  const testWebdav = useCallback(
    async (url: string, user: string, pass?: string) => {
      if (!backend) return false;
      setIsTestingWebdav(true);
      try {
        await backend.webdavTestConnection(url, user, pass || null);
        addToast({ message: t('settings.connection_successful'), type: 'success' });
        return true;
      } catch (err) {
        addToast({ message: `${t('settings.connection_failed')}: ${err}`, type: 'error' });
        return false;
      } finally {
        setIsTestingWebdav(false);
      }
    },
    [backend, addToast, t]
  );

  const syncWebdav = useCallback(
    async (url: string, user: string, pass?: string) => {
      if (!backend || !currentVault) return null;
      setIsSyncingWebdav(true);
      try {
        const stats = await backend.webdavSync(url, user, pass || null);
        if (stats.entries_added > 0 || stats.entries_updated > 0 || stats.trash_merged > 0) {
          addToast({
            message: `Cloud sync merged: ${stats.entries_added} added, ${stats.entries_updated} updated, ${stats.trash_merged} trashed.`,
            type: 'success',
          });
        } else {
          addToast({ message: t('settings.upload_backup'), type: 'success' });
        }
        return stats;
      } catch (err) {
        addToast({ message: t('toast.sync_failed', { err: String(err) }), type: 'error' });
        return null;
      } finally {
        setIsSyncingWebdav(false);
      }
    },
    [backend, currentVault, addToast, t]
  );

  const exportVaultFile = useCallback(async () => {
    if (!backend || !currentVault) return;
    try {
      const destPath = await saveFileDialog({
        defaultPath: `${currentVault.name}-backup.vdb`,
        filters: [{ name: 'Yntra Vault Database', extensions: ['vdb'] }],
      });
      if (!destPath) return;
      await backend.exportVault(destPath);
      addToast({ message: t('toast.encrypted_vault_exported'), type: 'success' });
    } catch (err) {
      addToast({ message: t('toast.export_failed', { err: String(err) }), type: 'error' });
    }
  }, [backend, currentVault, addToast, t]);

  const exportCsv = useCallback(async () => {
    if (!backend || !currentVault) return;
    try {
      const destPath = await saveFileDialog({
        defaultPath: `${currentVault.name}-passwords.csv`,
        filters: [{ name: 'CSV File', extensions: ['csv'] }],
      });
      if (!destPath) return;
      await backend.exportVaultCsv(destPath);
      addToast({ message: t('toast.decrypted_csv_exported'), type: 'success' });
    } catch (err) {
      addToast({ message: t('toast.export_failed', { err: String(err) }), type: 'error' });
    }
  }, [backend, currentVault, addToast, t]);

  const exportJson = useCallback(async () => {
    if (!backend || !currentVault) return;
    try {
      const destPath = await saveFileDialog({
        defaultPath: `${currentVault.name}-passwords.json`,
        filters: [{ name: 'JSON File', extensions: ['json'] }],
      });
      if (!destPath) return;
      await backend.exportVaultJson(destPath);
      addToast({ message: t('toast.decrypted_json_exported'), type: 'success' });
    } catch (err) {
      addToast({ message: t('toast.export_failed', { err: String(err) }), type: 'error' });
    }
  }, [backend, currentVault, addToast, t]);

  return {
    isTestingWebdav,
    isSyncingWebdav,
    isSyncingP2P,
    setIsSyncingP2P,
    testWebdav,
    syncWebdav,
    exportVaultFile,
    exportCsv,
    exportJson,
  };
}

export default useSync;
