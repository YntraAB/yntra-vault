import { useState, useCallback, useEffect, useRef, createContext, useContext, type ReactNode } from 'react';
import { appMetadata } from '@/lib/appMetadata';
import { UpdateModal } from './UpdateModal';
import { openExternalUrl } from '@/lib/utils';
import { useBackend } from '@/lib/useBackend';
import { useSettings } from '@/features/settings/context/SettingsContext';
import { useToast } from '@/contexts/ToastContext';
import { useTranslation } from '@/contexts/LanguageContext';
import type { CheckUpdateResult } from '@/types/ipc';

export type UpdateStatus =
  | 'idle'
  | 'checking'
  | 'available'
  | 'downloading'
  | 'ready'
  | 'up-to-date'
  | 'error';

function useUpdaterState() {
  const { backend } = useBackend();
  const { settings } = useSettings();
  const { addToast } = useToast();
  const { t } = useTranslation();

  const [status, setStatus] = useState<UpdateStatus>('idle');
  const [updateInfo, setUpdateInfo] = useState<CheckUpdateResult | null>(null);
  const [currentVersion, setCurrentVersion] = useState<string>('0.2.4');
  const [error, setError] = useState<string | null>(null);
  const [isModalOpen, setIsModalOpen] = useState(false);
  const [isDownloading, setIsDownloading] = useState(false);

  const hasAutoChecked = useRef(false);
  const operation = useRef(false);

  // Load current app version on mount
  useEffect(() => {
    if (!backend) return;
    backend.getAppVersion().then((v) => {
      if (v) setCurrentVersion(v);
    }).catch(() => {});
  }, [backend]);

  const checkForUpdates = useCallback(async (silent = false) => {
    if (!backend || operation.current) return;
    operation.current = true;
    hasAutoChecked.current = true;
    setStatus('checking');
    setError(null);

    try {
      const result = await backend.checkAppUpdate();
      setCurrentVersion(result.current_version);
      setUpdateInfo(result);

      if (result.has_update) {
        setStatus('available');
        if (!silent) {
          setIsModalOpen(true);
        } else {
          addToast({
            message: `${t('updater.update_available_toast') || 'New update available'}: v${result.latest_version}`,
            type: 'info',
          });
        }
      } else {
        setStatus('up-to-date');
        if (!silent) {
          addToast({
            message: t('updater.up_to_date_toast') || 'Yntra Vault is up to date.',
            type: 'success',
          });
        }
      }
      return result;
    } catch (err: unknown) {
      const errMsg = err instanceof Error ? err.message : String(err || 'Unknown updater error');
      setError(errMsg);
      setStatus('error');
      if (!silent) {
        addToast({
          message: `${t('updater.check_failed_toast') || 'Failed to check for updates'}: ${errMsg}`,
          type: 'error',
        });
      }
    }
    finally { operation.current = false; }
  }, [backend, addToast, t]);

  // Automatic background update check on app launch if enabled
  useEffect(() => {
    if (hasAutoChecked.current || !backend) return;
    if (settings.autoCheckUpdates === true && settings.operationMode !== 'airgap') {
      // Slight delay so initial vault UI loads smoothly
      const timer = setTimeout(() => {
        hasAutoChecked.current = true;
        checkForUpdates(true);
      }, 2500);
      return () => clearTimeout(timer);
    }
  }, [backend, settings.autoCheckUpdates, settings.operationMode, checkForUpdates]);

  const installUpdate = useCallback(async () => {
    if (!backend || !updateInfo?.has_update || !updateInfo.download_url || operation.current) return;
    operation.current = true;

    setIsDownloading(true);
    setStatus('downloading');
    setError(null);

    try {
      await appMetadata.flush();
      if (updateInfo.target_platform === 'android') {
        addToast({
          message: t('updater.downloading_apk_toast') || 'Downloading Android update package...',
          type: 'info',
        });

        await backend.downloadAndInstallApk(
          updateInfo.download_url,
          updateInfo.sha256 || undefined
        );

        setStatus('ready');
        addToast({
          message: t('updater.apk_ready_toast') || 'APK ready! Opening package installer...',
          type: 'success',
        });
      } else if (updateInfo.target_platform === 'windows-portable') {
        addToast({
          message: t('updater.updating_portable_toast') || 'Applying portable binary update...',
          type: 'info',
        });

        await backend.installPortableUpdate(
          updateInfo.download_url,
          updateInfo.sha256 || undefined
        );

        setStatus('ready');
        addToast({
          message: t('updater.portable_ready_toast') || 'Update applied successfully! Please restart Yntra Vault.',
          type: 'success',
        });
      } else {
        // Standard desktop packages are installed through the browser download.
        await openExternalUrl(updateInfo.download_url);
        setStatus('ready');
      }
      setIsModalOpen(false);
    } catch (err: unknown) {
      const errMsg = err instanceof Error ? err.message : String(err || 'Installation failed');
      setError(errMsg);
      setStatus('error');
      addToast({
        message: `${t('updater.install_failed_toast') || 'Update installation failed'}: ${errMsg}`,
        type: 'error',
      });
    } finally {
      operation.current = false;
      setIsDownloading(false);
    }
  }, [backend, updateInfo, addToast, t]);

  return {
    status,
    updateInfo,
    currentVersion,
    error,
    isModalOpen,
    isDownloading,
    setIsModalOpen,
    checkForUpdates,
    installUpdate,
  };
}

const UpdaterContext = createContext<ReturnType<typeof useUpdaterState> | null>(null);
export function UpdaterProvider({ children }: { children: ReactNode }) {
  const updater = useUpdaterState();
  return <UpdaterContext.Provider value={updater}>
    {children}
    <UpdateModal isOpen={updater.isModalOpen} onClose={() => updater.setIsModalOpen(false)}
      updateInfo={updater.updateInfo} currentVersion={updater.currentVersion}
      isDownloading={updater.isDownloading} onInstall={updater.installUpdate} error={updater.error} />
  </UpdaterContext.Provider>;
}
export function useUpdater() {
  const value = useContext(UpdaterContext);
  if (!value) throw new Error('useUpdater must be used within UpdaterProvider');
  return value;
}
