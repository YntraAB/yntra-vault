import { useState } from 'react';
import { FolderInput, AlertTriangle, FileSpreadsheet, FileCode } from 'lucide-react';
import { useAuth } from '@/features/auth';
import { useEntries } from '@/features/entries';
import { useSettings } from '../context/SettingsContext';
import { useToast } from '@/contexts/ToastContext';
import { useTranslation } from '@/contexts/LanguageContext';
import { useBackend } from '@/lib/useBackend';
import { saveFileDialog } from '@/lib/backend';
import { SettingSection, Toggle } from './SettingSection';

export interface BackupTabProps {
  onOpenImportModal: () => void;
}

export function BackupTab({ onOpenImportModal }: BackupTabProps) {
  const { currentVault } = useAuth();
  const { refreshEntries, refreshTags, setSelectedEntry } = useEntries();
  const { settings, updateSettings } = useSettings();
  const { addToast } = useToast();
  const { t } = useTranslation();
  const { backend } = useBackend();

  // WebDAV password kept in session storage so it persists during the app session
  const [webdavPass, setWebdavPass] = useState(() => {
    try {
      return sessionStorage.getItem('yntra-webdav-session-pass') || '';
    } catch {
      return '';
    }
  });

  const handleWebdavPassChange = (val: string) => {
    setWebdavPass(val);
    try {
      sessionStorage.setItem('yntra-webdav-session-pass', val);
    } catch {}
  };

  const [isTestingWebdav, setIsTestingWebdav] = useState(false);
  const [isSyncingWebdav, setIsSyncingWebdav] = useState(false);
  const [isRestoringWebdav, setIsRestoringWebdav] = useState(false);
  const [isSyncingP2P, setIsSyncingP2P] = useState(false);

  const webdavUrl = settings.webdavUrl ?? '';
  const webdavUser = settings.webdavUser ?? '';
  const webdavEnabled = Boolean(settings.webdavEnabled);
  const p2pAddr = settings.p2pAddr ?? '127.0.0.1:5322';

  return (
    <div className="flex flex-col gap-6">
      {/* Competitor Importer */}
      <SettingSection
        label={t('settings.importer_title')}
        tooltip={t('settings.tooltip_importer')}
      >
        <p className="mb-3 text-[12px] text-[var(--text-secondary)]">
          {t('settings.importer_desc')}
        </p>
        <button
          onClick={onOpenImportModal}
          className="flex h-8 items-center gap-1.5 rounded-[3px] bg-[var(--text-primary)] px-3.5 text-[12px] font-semibold text-[var(--bg-base)] transition-opacity hover:opacity-90 cursor-pointer"
        >
          <FolderInput size={14} />
          <span>{t('settings.import_passwords_btn')}</span>
        </button>
      </SettingSection>

      {/* WebDAV Cloud Sync */}
      <SettingSection
        label={t('settings.cloud_sync')}
        tooltip={t('settings.tooltip_cloud_sync')}
      >
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
            <Toggle
              checked={webdavEnabled}
              onChange={(v) => updateSettings({ webdavEnabled: v })}
            />
          </div>

          {webdavEnabled && (
            <div className="flex flex-col gap-3 rounded-[4px] border border-[var(--border)] bg-[var(--bg-card)] p-3">
              <div className="flex flex-col gap-2">
                <label className="text-[11px] font-semibold uppercase tracking-wider text-[var(--text-tertiary)]">
                  {t('settings.server_config')}
                </label>
                <input
                  type="text"
                  placeholder="https://nextcloud.example.com/remote.php/dav/files/user/vault.vdb"
                  value={webdavUrl}
                  onChange={(e) => updateSettings({ webdavUrl: e.target.value })}
                  className="h-8 w-full rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] px-2.5 text-[13px] text-[var(--text-primary)] outline-none focus:border-[var(--border-focus)]"
                />
                <div className="flex gap-2">
                  <input
                    type="text"
                    placeholder={t('detail.username')}
                    value={webdavUser}
                    onChange={(e) => updateSettings({ webdavUser: e.target.value })}
                    className="h-8 flex-1 rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] px-2.5 text-[13px] text-[var(--text-primary)] outline-none focus:border-[var(--border-focus)]"
                  />
                  <input
                    type="password"
                    placeholder={t('settings.app_password_token')}
                    value={webdavPass}
                    onChange={(e) => handleWebdavPassChange(e.target.value)}
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
                <Toggle
                  checked={Boolean(settings.webdavAutoSync)}
                  onChange={(v) => updateSettings({ webdavAutoSync: v })}
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
                  className="h-8 flex-1 rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] text-[12px] font-medium text-[var(--text-primary)] transition-colors hover:bg-[var(--bg-hover)] disabled:opacity-50 cursor-pointer"
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
                      addToast({ message: t('toast.sync_failed', { err: String(err) }), type: 'error' });
                    } finally {
                      setIsSyncingWebdav(false);
                    }
                  }}
                  className="h-8 flex-1 rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] text-[12px] font-medium text-[var(--text-primary)] transition-colors hover:bg-[var(--bg-hover)] disabled:opacity-50 cursor-pointer"
                >
                  {t('settings.sync_now')}
                </button>
                <button
                  disabled={isRestoringWebdav || isSyncingWebdav || isTestingWebdav}
                  onClick={async () => {
                    if (!backend || !currentVault) return;
                    if (!confirm(t('settings.restore_warning'))) return;
                    setIsRestoringWebdav(true);
                    try {
                      await backend.webdavDownload(webdavUrl, webdavUser, webdavPass || null, currentVault.path);
                      setSelectedEntry(null);
                      await Promise.all([refreshEntries(), refreshTags()]);
                      addToast({ message: t('toast.database_restored'), type: 'success' });
                    } catch (err) {
                      addToast({ message: t('toast.download_failed', { err: String(err) }), type: 'error' });
                    } finally {
                      setIsRestoringWebdav(false);
                    }
                  }}
                  className="h-8 flex-1 rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] text-[12px] font-medium text-[var(--text-primary)] transition-colors hover:bg-[var(--bg-hover)] text-amber-500 cursor-pointer disabled:opacity-50"
                >
                  {isRestoringWebdav ? `${t('settings.download_restore')}...` : t('settings.download_restore')}
                </button>
              </div>
            </div>
          )}
        </div>
      </SettingSection>

      {/* Local Network P2P Sync */}
      <SettingSection
        label={t('settings.p2p_sync')}
        tooltip={t('settings.tooltip_p2p_sync')}
      >
        <p className="mb-3 text-[12px] text-[var(--text-secondary)]">
          {t('settings.p2p_sync_desc')}
        </p>
        <div className="flex flex-col gap-2">
          <input
            type="text"
            placeholder={t('settings.p2p_placeholder')}
            value={p2pAddr}
            onChange={(e) => updateSettings({ p2pAddr: e.target.value })}
            className="h-8 w-full rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] px-2.5 text-[13px] text-[var(--text-primary)] outline-none focus:border-[var(--border-focus)]"
          />
          <div className="flex gap-2 mt-1">
            <button
              disabled={isSyncingP2P}
              onClick={async () => {
                if (!backend || !currentVault) return;
                setIsSyncingP2P(true);
                addToast({ message: t('toast.p2p_listening', { addr: p2pAddr }), type: 'info' });
                try {
                  await backend.runP2pSyncListener(p2pAddr, currentVault.path);
                  addToast({ message: t('toast.received_db_update'), type: 'success' });
                  await refreshEntries();
                } catch (err) {
                  addToast({ message: t('toast.sync_failed', { err: String(err) }), type: 'error' });
                } finally {
                  setIsSyncingP2P(false);
                }
              }}
              className="h-8 flex-1 rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] text-[12px] font-medium text-[var(--text-primary)] transition-colors hover:bg-[var(--bg-hover)] disabled:opacity-50 cursor-pointer"
            >
              {t('settings.listen_server')}
            </button>
            <button
              disabled={isSyncingP2P}
              onClick={async () => {
                if (!backend || !currentVault) return;
                setIsSyncingP2P(true);
                addToast({ message: t('toast.p2p_connecting', { addr: p2pAddr }), type: 'info' });
                try {
                  await backend.runP2pSyncClient(p2pAddr, currentVault.path);
                  addToast({ message: t('toast.received_db_update'), type: 'success' });
                  await refreshEntries();
                } catch (err) {
                  addToast({ message: t('toast.connection_failed', { err: String(err) }), type: 'error' });
                } finally {
                  setIsSyncingP2P(false);
                }
              }}
              className="h-8 flex-1 rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] text-[12px] font-medium text-[var(--text-primary)] transition-colors hover:bg-[var(--bg-hover)] disabled:opacity-50 cursor-pointer"
            >
              {t('settings.connect_client')}
            </button>
          </div>
        </div>
      </SettingSection>

      {/* Manual Export */}
      <SettingSection
        label={t('settings.manual_export')}
        tooltip={t('settings.tooltip_manual_export')}
      >
        <p className="mb-2.5 text-[12px] text-[var(--text-secondary)]">
          {t('settings.manual_export_desc')}
        </p>
        <div className="mb-3 flex items-center gap-2 rounded-[3px] border border-amber-500/30 bg-amber-500/10 p-2.5 text-[11px] text-amber-500 font-medium">
          <AlertTriangle size={14} className="shrink-0" />
          <span>{t('settings.export_warning')}</span>
        </div>
        <div className="flex flex-wrap items-center gap-2">
          {/* Encrypted Backup */}
          <button
            onClick={async () => {
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
            }}
            className="flex h-8 items-center gap-1.5 rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] px-3 text-[12px] font-medium text-[var(--text-primary)] transition-colors hover:bg-[var(--bg-hover)] cursor-pointer"
          >
            <FileSpreadsheet size={13} />
            <span>{t('settings.export_csv')}</span>
          </button>

          {/* JSON Export */}
          <button
            onClick={async () => {
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
            }}
            className="flex h-8 items-center gap-1.5 rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] px-3 text-[12px] font-medium text-[var(--text-primary)] transition-colors hover:bg-[var(--bg-hover)] cursor-pointer"
          >
            <FileCode size={13} />
            <span>{t('settings.export_json')}</span>
          </button>
        </div>
      </SettingSection>
    </div>
  );
}

export default BackupTab;
