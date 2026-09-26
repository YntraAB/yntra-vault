import { useState, useCallback, useEffect, useRef } from 'react';
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

export function useUpdater() {
  const { backend } = useBackend();
  const { settings } = useSettings();
  const { addToast } = useToast();
  const { t } = useTranslation();

  const [status, setStatus] = useState<UpdateStatus>('idle');
  const [updateInfo, setUpdateInfo] = useState<CheckUpdateResult | null>(null);
  const [currentVersion, setCurrentVersion] = useState<string>('0.2.3');
  const [error, setError] = useState<string | null>(null);
  const [isModalOpen, setIsModalOpen] = useState(false);
  const [isDownloading, setIsDownloading] = useState(false);

  const hasAutoChecked = useRef(false);

  // Load current app version on mount
  useEffect(() => {
    if (!backend) return;
    backend.getAppVersion().then((v) => {
      if (v) setCurrentVersion(v);
    }).catch(() => {});
  }, [backend]);

  const checkForUpdates = useCallback(async (silent = false) => {
    if (!backend) return;
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
  }, [backend, addToast, t]);

  // Automatic background update check on app launch if enabled
  useEffect(() => {
    if (hasAutoChecked.current || !backend) return;
    if (settings.autoCheckUpdates === true) {
      // Slight delay so initial vault UI loads smoothly
      const timer = setTimeout(() => {
        hasAutoChecked.current = true;
        checkForUpdates(true);
      }, 2500);
      return () => clearTimeout(timer);
    }
  }, [backend, settings.autoCheckUpdates, checkForUpdates]);

  const installUpdate = useCallback(async () => {
    if (!backend || !updateInfo || !updateInfo.download_url) return;

    setIsDownloading(true);
    setStatus('downloading');

    try {
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
    } catch (err: unknown) {
      const errMsg = err instanceof Error ? err.message : String(err || 'Installation failed');
      setError(errMsg);
      setStatus('error');
      addToast({
        message: `${t('updater.install_failed_toast') || 'Update installation failed'}: ${errMsg}`,
        type: 'error',
      });
    } finally {
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
