import { useState, useEffect, useCallback, useMemo } from 'react';
import { FolderInput, AlertTriangle, FileSpreadsheet, FileCode, Smartphone, Laptop, Plus, Unlink, Loader2, RefreshCw, X } from 'lucide-react';
import { useAuth } from '@/features/auth';
import { useEntries } from '@/features/entries';
import { useSettings } from '../context/SettingsContext';
import { useToast } from '@/contexts/ToastContext';
import { useTranslation } from '@/contexts/LanguageContext';
import { useBackend } from '@/lib/useBackend';
import { saveFileDialog, type TrustedDevice, type LocalDeviceInfo } from '@/lib/backend';
import { getTransientWebdavPassword, setTransientWebdavPassword } from '@/lib/sessionSecrets';
import { DevicePairingWizard, formatIpv4Input } from '@/features/sync';
import { SettingSection, Toggle } from './SettingSection';

export interface BackupTabProps {
  onOpenImportModal: () => void;
}

export function BackupTab({ onOpenImportModal }: BackupTabProps) {
  const { currentVault } = useAuth();
  const { refreshEntries, refreshTags, setSelectedEntry, toggleP2pListener } = useEntries();
  const { settings, updateSettings } = useSettings();
  const { addToast } = useToast();
  const { t } = useTranslation();
  const { backend } = useBackend();

  // WebDAV password held strictly in transient memory (no sessionStorage persistence)
  const [webdavPass, setWebdavPass] = useState(() => getTransientWebdavPassword() || '');

  const handleWebdavPassChange = (val: string) => {
    setWebdavPass(val);
    setTransientWebdavPassword(val || null);
  };

  const [isTestingWebdav, setIsTestingWebdav] = useState(false);
  const [isSyncingWebdav, setIsSyncingWebdav] = useState(false);
  const [isRestoringWebdav, setIsRestoringWebdav] = useState(false);
  const [showPairingWizard, setShowPairingWizard] = useState(false);

  // Trusted Devices State
  const [trustedDevices, setTrustedDevices] = useState<TrustedDevice[]>([]);
  const [isLoadingDevices, setIsLoadingDevices] = useState(false);
  const [revokingDeviceId, setRevokingDeviceId] = useState<string | null>(null);

  const loadTrustedDevices = useCallback(async () => {
    if (!backend || !currentVault) return;
    setIsLoadingDevices(true);
    try {
      const list = await backend.getTrustedDevices();
      setTrustedDevices(list);
    } catch (e) {
      console.error('Failed to fetch trusted devices:', e);
    } finally {
      setIsLoadingDevices(false);
    }
  }, [backend, currentVault]);

  useEffect(() => {
    loadTrustedDevices();
  }, [loadTrustedDevices]);

  const [localDevice, setLocalDevice] = useState<LocalDeviceInfo | null>(null);
  const [syncingDeviceId, setSyncingDeviceId] = useState<string | null>(null);
  const [ipPromptDevice, setIpPromptDevice] = useState<TrustedDevice | null>(null);
  const [promptIpValue, setPromptIpValue] = useState<string>('');
  const [isPromptSubmitting, setIsPromptSubmitting] = useState(false);

  useEffect(() => {
    if (backend) {
      backend.getLocalDeviceInfo().then(setLocalDevice).catch(() => {});
    }
  }, [backend]);

  const uniqueDevices = useMemo(() => {
    const seenIds = new Set<string>();
    const seenNames = new Set<string>();
    const list: TrustedDevice[] = [];
    for (const dev of trustedDevices) {
      const nameKey = `${(dev.name || '').toLowerCase()}:${(dev.os || '').toLowerCase()}`;
      if (!seenIds.has(dev.id) && !seenNames.has(nameKey)) {
        seenIds.add(dev.id);
        seenNames.add(nameKey);
        list.push(dev);
      }
    }
    return list;
  }, [trustedDevices]);

  const handleSyncDevice = async (device: TrustedDevice, targetIpOverride?: string) => {
    if (!backend || !currentVault) return;

    let targetAddr = targetIpOverride?.trim() || '';

    if (!targetAddr) {
      const cached = localStorage.getItem(`yntra_peer_addr_${device.id}`) || localStorage.getItem('yntra_last_peer_addr');
      if (cached) {
        targetAddr = cached.trim();
      }
    }

    if (!targetAddr) {
      setSyncingDeviceId(device.id);
      try {
        const discovered = await backend.scanP2pDiscovery(1500);
        if (discovered) {
          targetAddr = discovered;
        }
      } catch {
        // ignore discovery error
      }
    }

    if (!targetAddr) {
      setSyncingDeviceId(null);
      setPromptIpValue(localStorage.getItem('yntra_last_peer_addr') || '');
      setIpPromptDevice(device);
      return;
    }

    setSyncingDeviceId(device.id);
    try {
      const hostPart = targetAddr.split(':')[0].trim();
      const cleanAddr = `${hostPart}:5322`;
      const stats = await backend.runP2pSyncClient(cleanAddr, currentVault.path, device.id);

      localStorage.setItem(`yntra_peer_addr_${device.id}`, hostPart);
      localStorage.setItem('yntra_last_peer_addr', hostPart);

      await Promise.all([refreshEntries(), refreshTags(), loadTrustedDevices()]);

      const updatedCount = stats.entries_added + stats.entries_updated;
      addToast({
        message: updatedCount > 0
          ? t('settings.sync_success_count', { count: updatedCount })
          : t('settings.sync_success'),
        type: 'success',
      });
      setIpPromptDevice(null);
    } catch (err: any) {
      const errMsg = typeof err === 'string' ? err : err?.message || String(err);
      if (!targetIpOverride) {
        setPromptIpValue(targetAddr.split(':')[0].trim() || '');
        setIpPromptDevice(device);
      } else {
        addToast({
          message: t('settings.sync_failed', { name: device.name || 'Enhet', err: errMsg }),
          type: 'error',
        });
      }
    } finally {
      setSyncingDeviceId(null);
    }
  };

  const handleRevokeDevice = async (device: TrustedDevice) => {
    if (!backend) return;
    const confirmMsg = t('settings.revoke_device_confirm', { name: device.name || 'Enhet' });
    if (!confirm(confirmMsg)) return;

    setRevokingDeviceId(device.id);
    try {
      await backend.revokeTrustedDevice(device.id);
      addToast({
        message: t('settings.revoke_device_success', { name: device.name || 'Enhet' }),
        type: 'success',
      });
      await loadTrustedDevices();
    } catch (err: any) {
      addToast({
        message: t('settings.revoke_device_failed', { err: typeof err === 'string' ? err : err?.message || String(err) }),
        type: 'error',
      });
    } finally {
      setRevokingDeviceId(null);
    }
  };

  const webdavUrl = settings.webdavUrl ?? '';
  const webdavUser = settings.webdavUser ?? '';
  const webdavEnabled = Boolean(settings.webdavEnabled);

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
                      await Promise.all([refreshEntries(), refreshTags()]);
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

      {/* Unified Trusted Devices (Kopplade enheter) */}
      <SettingSection
        label={t('settings.trusted_devices_title')}
        tooltip={t('settings.trusted_devices_desc')}
      >
        <p className="mb-3.5 text-[12px] text-[var(--text-secondary)]">
          {t('settings.trusted_devices_desc')}
        </p>

        {/* Action Bar / Header */}
        <div className="mb-3 flex items-center justify-between">
          <div className="flex items-center gap-2">
            <span className="text-[11px] font-semibold tracking-wider text-[var(--text-tertiary)] uppercase">
              {trustedDevices.length > 0 ? `${trustedDevices.length} ${trustedDevices.length === 1 ? 'enhet' : 'enheter'}` : ''}
            </span>
            {isLoadingDevices && <Loader2 size={12} className="animate-spin text-[var(--text-tertiary)]" />}
          </div>
          <button
            type="button"
            onClick={() => setShowPairingWizard(true)}
            className="flex h-8 items-center gap-1.5 rounded-md bg-[var(--text-primary)] px-3 text-[12px] font-semibold text-[var(--bg-base)] hover:opacity-90 transition-opacity cursor-pointer shrink-0"
          >
            <Plus size={14} />
            <span>{t('settings.pair_new_device_btn')}</span>
          </button>
        </div>

        {/* Devices List or Empty State */}
        <div className="space-y-2">
          {uniqueDevices.length === 0 ? (
            <div className="flex flex-col items-center justify-center rounded-lg border border-dashed border-[var(--border)] bg-[var(--bg-elevated)] p-6 text-center">
              <div className="flex h-10 w-10 items-center justify-center rounded-full bg-[var(--bg-base)] border border-[var(--border)] text-[var(--text-tertiary)] mb-2.5">
                <Smartphone size={20} />
              </div>
              <h4 className="text-[13px] font-semibold text-[var(--text-primary)]">
                {t('settings.no_trusted_devices')}
              </h4>
              <p className="text-[11px] text-[var(--text-secondary)] max-w-sm mt-1 leading-relaxed">
                {t('settings.no_trusted_devices_desc')}
              </p>
            </div>
          ) : (
            uniqueDevices.map((device) => {
              const isMobile = device.device_type?.toLowerCase().includes('mobile') ||
                device.os?.toLowerCase().includes('android') ||
                device.os?.toLowerCase().includes('ios');
              const pairedDate = device.paired_at ? new Date(device.paired_at).toLocaleDateString() : '';
              const lastSync = device.last_sync_at ? new Date(device.last_sync_at).toLocaleString() : null;
              const isCurrentDevice = Boolean(
                localDevice && (
                  device.id === localDevice.id || (
                    device.name.toLowerCase() === localDevice.name.toLowerCase() &&
                    device.os.toLowerCase() === localDevice.os.toLowerCase()
                  )
                )
              );

              return (
                <div
                  key={device.id}
                  className="flex items-center justify-between gap-3.5 rounded-lg border border-[var(--border)] bg-[var(--bg-elevated)] p-3 transition-colors hover:border-[var(--border-subtle)]"
                >
                  <div className="flex items-center gap-3 min-w-0 flex-1">
                    <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-md border border-[var(--border)] bg-[var(--bg-base)] text-[var(--text-primary)]">
                      {isMobile ? <Smartphone size={18} /> : <Laptop size={18} />}
                    </div>
                    <div className="min-w-0 flex-1">
                      <div className="flex items-center gap-2">
                        <span className="text-[13px] font-semibold text-[var(--text-primary)] truncate">
                          {device.name || 'Enhet'}
                        </span>
                        {device.os && (
                          <span className="inline-flex items-center rounded px-1.5 py-0.5 text-[10px] font-medium bg-[var(--bg-base)] border border-[var(--border)] text-[var(--text-secondary)]">
                            {device.os}
                          </span>
                        )}
                        {isCurrentDevice && (
                          <span className="inline-flex items-center rounded px-1.5 py-0.5 text-[10px] font-medium bg-[var(--bg-base)] border border-[var(--border)] text-[var(--text-tertiary)]">
                            {t('settings.this_device')}
                          </span>
                        )}
                      </div>
                      <div className="flex items-center gap-2 mt-0.5 text-[11px] text-[var(--text-secondary)]">
                        {pairedDate && <span>{t('settings.device_paired_on', { date: pairedDate })}</span>}
                        {pairedDate && <span>•</span>}
                        <span>
                          {lastSync ? t('settings.device_last_synced', { date: lastSync }) : t('settings.device_never_synced')}
                        </span>
                      </div>
                    </div>
                  </div>

                  {!isCurrentDevice && (
                    <div className="flex items-center gap-2 shrink-0">
                      <button
                        type="button"
                        disabled={syncingDeviceId === device.id}
                        onClick={() => handleSyncDevice(device)}
                        className="flex h-7.5 items-center gap-1.5 rounded-md bg-[var(--text-primary)] px-2.5 text-[11px] font-semibold text-[var(--bg-base)] hover:opacity-90 transition-opacity cursor-pointer disabled:opacity-50"
                        title={t('settings.sync_now_btn')}
                      >
                        <RefreshCw size={12} className={syncingDeviceId === device.id ? 'animate-spin' : ''} />
                        <span>{syncingDeviceId === device.id ? t('settings.syncing') : t('settings.sync_now_btn')}</span>
                      </button>

                      <button
                        type="button"
                        disabled={revokingDeviceId === device.id}
                        onClick={() => handleRevokeDevice(device)}
                        className="flex h-7.5 items-center gap-1.5 rounded-md border border-rose-500/30 bg-rose-500/10 px-2.5 text-[11px] font-medium text-rose-500 hover:bg-rose-500/20 transition-colors cursor-pointer disabled:opacity-50"
                        title={t('settings.revoke_device_btn')}
                      >
                        <Unlink size={12} />
                        <span>{t('settings.revoke_device_btn')}</span>
                      </button>
                    </div>
                  )}
                </div>
              );
            })
          )}
        </div>

        {/* Automatic Wi-Fi Sync Toggle */}
        <div className="mt-4 border-t border-[var(--border)] pt-3.5">
          <div className="flex items-center justify-between gap-4">
            <div className="flex flex-col min-w-0 flex-1">
              <span className="text-[12px] font-medium text-[var(--text-primary)]">
                {t('settings.auto_wifi_sync_toggle')}
              </span>
              <span className="text-[11px] text-[var(--text-secondary)] leading-relaxed mt-0.5">
                {t('settings.auto_wifi_sync_desc')}
              </span>
            </div>
            <Toggle
              checked={Boolean(settings.p2pAutoSyncWifi && settings.p2pAutoListen)}
              onChange={(v) => {
                updateSettings({ p2pAutoSyncWifi: v, p2pAutoListen: v });
                toggleP2pListener(v);
              }}
            />
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

      <DevicePairingWizard
        isOpen={showPairingWizard}
        onClose={() => setShowPairingWizard(false)}
        onSuccess={() => {
          Promise.all([refreshEntries(), refreshTags()]).catch(() => {});
          loadTrustedDevices();
        }}
      />

      {/* Manual IP Prompt Modal for 1-Click Sync */}
      {ipPromptDevice && (
        <div
          className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 select-none p-4"
          onClick={() => setIpPromptDevice(null)}
        >
          <div
            className="w-full max-w-[400px] rounded-lg border border-[var(--border)] bg-[var(--bg-base)] shadow-2xl overflow-hidden flex flex-col p-5"
            onClick={(e) => e.stopPropagation()}
          >
            <div className="flex items-center justify-between border-b border-[var(--border-subtle)] pb-3 mb-3">
              <h3 className="text-[14px] font-semibold text-[var(--text-primary)]">
                {t('settings.prompt_device_ip_title', { name: ipPromptDevice.name || 'Device' })}
              </h3>
              <button
                type="button"
                onClick={() => setIpPromptDevice(null)}
                className="rounded-md p-1 text-[var(--text-tertiary)] hover:text-[var(--text-primary)] cursor-pointer"
              >
                <X size={16} />
              </button>
            </div>
            <p className="text-[12px] text-[var(--text-secondary)] mb-3 leading-relaxed">
              {t('settings.prompt_device_ip_desc')}
            </p>
            <div className="flex flex-col gap-1.5 mb-4">
              <input
                type="text"
                autoFocus
                value={promptIpValue}
                onChange={(e) => setPromptIpValue(formatIpv4Input(e.target.value, promptIpValue))}
                onKeyDown={(e) => {
                  if (e.key === 'Enter' && promptIpValue.trim()) {
                    handleSyncDevice(ipPromptDevice, promptIpValue);
                  }
                }}
                placeholder={t('settings.prompt_device_ip_ph')}
                className="h-9 w-full rounded-md border border-[var(--border)] bg-[var(--bg-elevated)] px-3 font-mono text-[13px] text-[var(--text-primary)] outline-none focus:border-[var(--border-focus)]"
              />
            </div>
            <div className="flex items-center justify-end gap-2">
              <button
                type="button"
                onClick={() => setIpPromptDevice(null)}
                className="h-8 rounded-md px-3 text-[12px] font-medium text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] cursor-pointer"
              >
                {t('common.cancel') || 'Cancel'}
              </button>
              <button
                type="button"
                disabled={!promptIpValue.trim() || isPromptSubmitting}
                onClick={async () => {
                  setIsPromptSubmitting(true);
                  await handleSyncDevice(ipPromptDevice, promptIpValue);
                  setIsPromptSubmitting(false);
                }}
                className="flex h-8 items-center gap-1.5 rounded-md bg-[var(--text-primary)] px-3.5 text-[12px] font-semibold text-[var(--bg-base)] hover:opacity-90 transition-opacity cursor-pointer disabled:opacity-50"
              >
                {isPromptSubmitting ? <Loader2 size={13} className="animate-spin" /> : <RefreshCw size={13} />}
                <span>{t('settings.connect_btn')}</span>
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

export default BackupTab;
