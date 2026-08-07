import { useState } from 'react';
import { FolderInput, AlertTriangle, FileSpreadsheet, FileCode } from 'lucide-react';
import { useAppState } from '@/contexts/AppStateContext';
import { useTranslation } from '@/contexts/LanguageContext';
import { useBackend } from '@/lib/useBackend';
import { SettingSection } from './SettingSection';

interface BackupTabProps {
  onOpenImportModal: () => void;
}

export function BackupTab({ onOpenImportModal }: BackupTabProps) {
  const { currentVault, refreshEntries, addToast } = useAppState();
  const { t } = useTranslation();
  const { backend } = useBackend();

  // WebDAV state
  const [webdavEnabled, setWebdavEnabled] = useState(false);
  const [autoSyncOnSave, setAutoSyncOnSave] = useState(false);
  const [webdavUrl, setWebdavUrl] = useState('');
  const [webdavUser, setWebdavUser] = useState('');
  const [webdavPass, setWebdavPass] = useState('');
  const [isTestingWebdav, setIsTestingWebdav] = useState(false);
  const [isSyncingWebdav, setIsSyncingWebdav] = useState(false);

  // P2P state
  const [p2pAddr, setP2pAddr] = useState('127.0.0.1:5322');
  const [isSyncingP2P, setIsSyncingP2P] = useState(false);

  return (
    <div className="flex flex-col gap-6">
      {/* WebDAV Cloud Sync */}
      <SettingSection label={t('settings.cloud_sync')}>
        <p className="mb-3 text-[12px] text-[var(--text-secondary)]">
          {t('settings.cloud_sync_desc')}
        </p>
        <div className="flex flex-col gap-4">
          {/* Opt-In Master Toggle */}
          <div className="flex items-center justify-between rounded-[4px] border border-[var(--border)] bg-[var(--bg-elevated)] p-3">
            <div className="flex flex-col gap-0.5">
              <span className="text-[13px] font-medium text-[var(--text-primary)]">
                {t('settings.enable_webdav')}
              </span>
              <span className="text-[11px] text-[var(--text-secondary)]">
                {t('settings.enable_webdav_desc')}
              </span>
            </div>
            <input
              type="checkbox"
              checked={webdavEnabled}
              onChange={(e) => setWebdavEnabled(e.target.checked)}
              className="h-4 w-4 rounded border-[var(--border)] accent-[var(--accent)] cursor-pointer"
            />
          </div>

          {webdavEnabled && (
            <div className="flex flex-col gap-3 rounded-[4px] border border-[var(--border)] bg-[var(--bg-card)] p-3">
              <div className="flex flex-col gap-2">
                <label className="text-[11px] font-semibold uppercase tracking-wider text-[var(--text-tertiary)]">
                  Server Configuration
                </label>
                <input
                  type="text"
                  placeholder="https://nextcloud.example.com/remote.php/dav/files/user/vault.vdb"
                  value={webdavUrl}
                  onChange={(e) => setWebdavUrl(e.target.value)}
                  className="h-8 w-full rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] px-2.5 text-[13px] text-[var(--text-primary)] outline-none focus:border-[var(--border-focus)]"
                />
                <div className="flex gap-2">
                  <input
                    type="text"
                    placeholder={t('detail.username')}
                    value={webdavUser}
                    onChange={(e) => setWebdavUser(e.target.value)}
                    className="h-8 flex-1 rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] px-2.5 text-[13px] text-[var(--text-primary)] outline-none focus:border-[var(--border-focus)]"
                  />
                  <input
                    type="password"
                    placeholder="App Password / Token"
                    value={webdavPass}
                    onChange={(e) => setWebdavPass(e.target.value)}
                    className="h-8 flex-1 rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] px-2.5 text-[13px] text-[var(--text-primary)] outline-none focus:border-[var(--border-focus)]"
                  />
                </div>
              </div>

              <div className="flex items-center justify-between pt-1 border-t border-[var(--border)]">
                <div className="flex flex-col">
                  <span className="text-[12px] font-medium text-[var(--text-primary)]">
                    {t('settings.auto_sync_on_save')}
                  </span>
                  <span className="text-[10px] text-[var(--text-secondary)]">
                    {t('settings.auto_sync_on_save_desc')}
                  </span>
                </div>
                <input
                  type="checkbox"
                  checked={autoSyncOnSave}
                  onChange={(e) => setAutoSyncOnSave(e.target.checked)}
                  className="h-4 w-4 rounded border-[var(--border)] accent-[var(--accent)] cursor-pointer"
                />
              </div>

              <div className="flex gap-2 pt-2 border-t border-[var(--border)]">
                <button
                  disabled={isTestingWebdav}
                  onClick={async () => {
                    if (!backend) return;
                    setIsTestingWebdav(true);
                    try {
                      await backend.webdavTestConnection(webdavUrl, webdavUser, webdavPass || null);
                      addToast({ message: t('settings.connection_successful'), type: 'success' });
                    } catch (err) {
                      addToast({ message: `${t('settings.connection_failed')}: ${err}`, type: 'error' });
                    } finally {
                      setIsTestingWebdav(false);
                    }
                  }}
                  className="h-8 flex-1 rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] text-[12px] font-medium text-[var(--text-primary)] transition-colors hover:bg-[var(--bg-hover)] disabled:opacity-50"
                >
                  {isTestingWebdav ? t('settings.testing_connection') : t('settings.test_connection')}
                </button>
                <button
                  disabled={isSyncingWebdav}
                  onClick={async () => {
                    if (!backend || !currentVault) return;
                    setIsSyncingWebdav(true);
                    try {
                      const stats = await backend.webdavSync(
                        webdavUrl,
                        webdavUser,
                        webdavPass || null
                      );
                      await refreshEntries();
                      if (stats.entries_added > 0 || stats.entries_updated > 0 || stats.trash_merged > 0) {
                        addToast({
                          message: `Cloud sync merged: ${stats.entries_added} added, ${stats.entries_updated} updated, ${stats.trash_merged} trashed.`,
                          type: 'success'
                        });
                      } else {
                        addToast({ message: t('settings.upload_backup'), type: 'success' });
                      }
                    } catch (err: any) {
                      addToast({ message: `Sync failed: ${err}`, type: 'error' });
                    } finally {
                      setIsSyncingWebdav(false);
                    }
                  }}
                  className="h-8 flex-1 rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] text-[12px] font-medium text-[var(--text-primary)] transition-colors hover:bg-[var(--bg-hover)] disabled:opacity-50"
                >
                  {t('settings.sync_now')}
                </button>
                <button
                  onClick={async () => {
                    if (!backend || !currentVault) return;
                    if (!confirm(t('settings.restore_warning'))) return;
                    try {
                      await backend.webdavDownload(webdavUrl, webdavUser, webdavPass || null, currentVault.path);
                      addToast({ message: 'Database restored from backup! Safety backup created (.vdb.bak)', type: 'success' });
                      await refreshEntries();
                    } catch (err) {
                      addToast({ message: `Download failed: ${err}`, type: 'error' });
                    }
                  }}
                  className="h-8 flex-1 rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] text-[12px] font-medium text-[var(--text-primary)] transition-colors hover:bg-[var(--bg-hover)] text-amber-500"
                >
                  {t('settings.download_restore')}
                </button>
              </div>
            </div>
          )}
        </div>
      </SettingSection>

      {/* Local Network P2P Sync */}
      <SettingSection label={t('settings.p2p_sync')}>
        <p className="mb-3 text-[12px] text-[var(--text-secondary)]">
          {t('settings.p2p_sync_desc')}
        </p>
        <div className="flex flex-col gap-2">
          <input
            type="text"
            placeholder="IP Address:Port (e.g. 192.168.1.50:5322)"
            value={p2pAddr}
            onChange={(e) => setP2pAddr(e.target.value)}
            className="h-8 w-full rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] px-2.5 text-[13px] text-[var(--text-primary)] outline-none focus:border-[var(--border-focus)]"
          />
          <div className="flex gap-2 mt-1">
            <button
              disabled={isSyncingP2P}
              onClick={async () => {
                if (!backend || !currentVault) return;
                setIsSyncingP2P(true);
                addToast({ message: `Listening for P2P connection on ${p2pAddr}...`, type: 'info' });
                try {
                  await backend.runP2pSyncListener(p2pAddr, currentVault.path);
                  addToast({ message: 'Received database update successfully!', type: 'success' });
                  await refreshEntries();
                } catch (err) {
                  addToast({ message: `Sync failed: ${err}`, type: 'error' });
                } finally {
                  setIsSyncingP2P(false);
                }
              }}
              className="h-8 flex-1 rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] text-[12px] font-medium text-[var(--text-primary)] transition-colors hover:bg-[var(--bg-hover)] disabled:opacity-50"
            >
              {t('settings.listen_server')}
            </button>
            <button
              disabled={isSyncingP2P}
              onClick={async () => {
                if (!backend || !currentVault) return;
                setIsSyncingP2P(true);
                addToast({ message: `Connecting to ${p2pAddr}...`, type: 'info' });
                try {
                  await backend.runP2pSyncClient(p2pAddr, currentVault.path);
                  addToast({ message: 'Database sync sent successfully!', type: 'success' });
                } catch (err) {
                  addToast({ message: `Connection failed: ${err}`, type: 'error' });
                } finally {
                  setIsSyncingP2P(false);
                }
              }}
              className="h-8 flex-1 rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] text-[12px] font-medium text-[var(--text-primary)] transition-colors hover:bg-[var(--bg-hover)] disabled:opacity-50"
            >
              {t('settings.connect_client')}
            </button>
          </div>
        </div>
      </SettingSection>

      {/* Competitor Importer */}
      <SettingSection label="Competitor Password Importer">
        <p className="mb-3 text-[12px] text-[var(--text-secondary)]">
          Import passwords and logins from Bitwarden, 1Password, KeePass, Chrome, LastPass, Dashlane, Proton Pass, or generic CSV.
        </p>
        <button
          onClick={onOpenImportModal}
          className="flex h-8 items-center gap-1.5 rounded-[3px] bg-[var(--text-primary)] px-3.5 text-[12px] font-semibold text-[var(--bg-base)] transition-opacity hover:opacity-90 cursor-pointer"
        >
          <FolderInput size={14} />
          <span>Import Passwords</span>
        </button>
      </SettingSection>

      {/* Manual Export */}
      <SettingSection label={t('settings.manual_export')}>
        <p className="mb-2.5 text-[12px] text-[var(--text-secondary)]">
          {t('settings.manual_export_desc')}
        </p>
        <div className="mb-3 flex items-center gap-2 rounded-[3px] border border-amber-500/30 bg-amber-500/10 p-2.5 text-[11px] text-amber-500 font-medium">
          <AlertTriangle size={14} className="shrink-0" />
          <span>Warning: CSV and JSON exports contain unencrypted credentials. Keep exported files secure and delete them after migration.</span>
        </div>
        <div className="flex flex-wrap items-center gap-2">
          {/* Encrypted Backup */}
          <button
            onClick={async () => {
              if (!backend || !currentVault) return;
              try {
                const { save } = await import('@tauri-apps/plugin-dialog');
                const destPath = await save({
                  defaultPath: `${currentVault.name}-backup.vdb`,
                  filters: [{ name: 'Yntra Vault Database', extensions: ['vdb'] }],
                });
                if (!destPath) return;
                await backend.exportVault(destPath);
                addToast({ message: 'Encrypted vault exported successfully!', type: 'success' });
              } catch (err) {
                addToast({ message: `Export failed: ${err}`, type: 'error' });
              }
            }}
            className="h-8 rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] px-3 text-[12px] font-medium text-[var(--text-primary)] transition-colors hover:bg-[var(--bg-hover)] cursor-pointer"
          >
            {t('settings.export_file')} (.vdb)
          </button>

          {/* CSV Export */}
          <button
            onClick={async () => {
              if (!backend || !currentVault) return;
              try {
                const { save } = await import('@tauri-apps/plugin-dialog');
                const destPath = await save({
                  defaultPath: `${currentVault.name}-passwords.csv`,
                  filters: [{ name: 'CSV File', extensions: ['csv'] }],
                });
                if (!destPath) return;
                await backend.exportVaultCsv(destPath);
                addToast({ message: 'Decrypted CSV exported successfully!', type: 'success' });
              } catch (err) {
                addToast({ message: `Export failed: ${err}`, type: 'error' });
              }
            }}
            className="flex h-8 items-center gap-1.5 rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] px-3 text-[12px] font-medium text-[var(--text-primary)] transition-colors hover:bg-[var(--bg-hover)] cursor-pointer"
          >
            <FileSpreadsheet size={13} />
            <span>Export CSV</span>
          </button>

          {/* JSON Export */}
          <button
            onClick={async () => {
              if (!backend || !currentVault) return;
              try {
                const { save } = await import('@tauri-apps/plugin-dialog');
                const destPath = await save({
                  defaultPath: `${currentVault.name}-passwords.json`,
                  filters: [{ name: 'JSON File', extensions: ['json'] }],
                });
                if (!destPath) return;
                await backend.exportVaultJson(destPath);
                addToast({ message: 'Decrypted JSON exported successfully!', type: 'success' });
              } catch (err) {
                addToast({ message: `Export failed: ${err}`, type: 'error' });
              }
            }}
            className="flex h-8 items-center gap-1.5 rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] px-3 text-[12px] font-medium text-[var(--text-primary)] transition-colors hover:bg-[var(--bg-hover)] cursor-pointer"
          >
            <FileCode size={13} />
            <span>Export JSON</span>
          </button>
        </div>
      </SettingSection>
    </div>
  );
}
