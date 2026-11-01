import { appMetadata } from '@/lib/appMetadata';
import { getBackend,isTauri } from '@/lib/backend';
import { DEFAULT_KEYBINDS } from '@/lib/keybinds';
import type { AppSettings } from '@/types';
import React,{ useCallback,useEffect,useMemo,useRef,useState } from 'react';
import { DEFAULT_SETTINGS,SettingsContext } from './SettingsContext.model';


export function SettingsProvider({ children }: { children: React.ReactNode }) {
  const [externalFaviconsReady, setExternalFaviconsReady] = useState(false);
  const [networkAccessReady, setNetworkAccessReady] = useState(false);
  const faviconSync = useRef(Promise.resolve());
  const [settings, setSettings] = useState<AppSettings>(() => {
    try {
      const saved = appMetadata.getItem('yntra-vault-settings');
      if (saved) {
        const parsed = JSON.parse(saved);
        return {
          ...DEFAULT_SETTINGS,
          ...parsed,
          keybinds: {
            ...DEFAULT_KEYBINDS,
            ...(parsed.keybinds || {}),
          },
        };
      }
    } catch (e) {
      console.warn('Failed to load settings from localStorage:', e);
    }
    return DEFAULT_SETTINGS;
  });

  const updateSettings = useCallback((partial: Partial<AppSettings>) => {
    if (partial.operationMode !== undefined && partial.operationMode !== settings.operationMode) {
      setNetworkAccessReady(false);
    }
    if (partial.operationMode !== undefined || partial.externalFaviconsEnabled !== undefined) {
      setExternalFaviconsReady(false);
    }
    setSettings((prev) => {
      const next = { ...prev, ...partial };
      try {
        appMetadata.setItem('yntra-vault-settings', JSON.stringify(next));
      } catch (e) {
        console.warn('Failed to save settings to localStorage:', e);
      }
      return next;
    });
  }, [settings.operationMode]);

  // Sync minimizeToTray setting to backend
  useEffect(() => {
    if (isTauri()) {
      getBackend().then((b) => {
        b.setMinimizeToTray(settings.minimizeToTray !== false).catch((err) => {
          console.error('Failed to sync minimizeToTray setting:', err);
        });
      }).catch(() => {});
    }
  }, [settings.minimizeToTray]);

  // Sync windowCaptureProtection setting to backend
  useEffect(() => {
    if (isTauri()) {
      getBackend().then((b) => {
        b.setWindowCaptureProtection(settings.windowCaptureProtection !== false).catch((err) => {
          console.error('Failed to sync windowCaptureProtection setting:', err);
        });
      }).catch(() => {});
    }
  }, [settings.windowCaptureProtection]);

  // Sync lockOnFocusLoss setting to backend
  useEffect(() => {
    if (isTauri()) {
      getBackend().then((b) => {
        b.setLockOnFocusLoss(settings.lockOnFocusLoss === true).catch((err) => {
          console.error('Failed to sync lockOnFocusLoss setting:', err);
        });
      }).catch(() => {});
    }
  }, [settings.lockOnFocusLoss]);

  // Sync lockOnSystemLock setting to backend
  useEffect(() => {
    if (isTauri()) {
      getBackend().then((b) => {
        b.setLockOnSystemLock(settings.lockOnSystemLock !== false).catch((err) => {
          console.error('Failed to sync lockOnSystemLock setting:', err);
        });
      }).catch(() => {});
    }
  }, [settings.lockOnSystemLock]);

  // Sync externalFaviconsEnabled setting to backend and notify UI cache
  useEffect(() => {
    const networkEnabled = settings.operationMode !== 'airgap';
    const isEnabled = networkEnabled && settings.externalFaviconsEnabled === true;
    let cancelled = false;
    if (!isEnabled) window.dispatchEvent(new CustomEvent('yntra-favicons-cleared'));
    // Serialize toggles and wait for the native gate before any icon request.
    faviconSync.current = faviconSync.current.catch(() => {}).then(async () => {
      if (cancelled) return;
      if (isTauri()) {
        const backend = await getBackend();
        await backend.setNetworkAccess(networkEnabled);
        if (!cancelled) setNetworkAccessReady(networkEnabled);
        await backend.setExternalFaviconsEnabled(isEnabled);
      }
      if (!cancelled) {
        setNetworkAccessReady(networkEnabled);
        setExternalFaviconsReady(isEnabled);
        if (isEnabled) window.dispatchEvent(new CustomEvent('yntra-favicons-reset'));
      }
    }).catch(err => {
      if (!cancelled) { setNetworkAccessReady(false); setExternalFaviconsReady(false); }
      console.error('Failed to apply network privacy setting:', err);
    });
    return () => { cancelled = true; };
  }, [settings.externalFaviconsEnabled, settings.operationMode]);

  // Apply font size & density globally to document root
  useEffect(() => {
    if (typeof document !== 'undefined') {
      if (settings.fontSize) {
        document.documentElement.style.fontSize = `${settings.fontSize}px`;
      }
      document.documentElement.setAttribute('data-density', settings.density || 'normal');
    }
  }, [settings.fontSize, settings.density]);

  const value = useMemo(() => ({ settings, updateSettings, externalFaviconsReady, networkAccessReady }), [settings, updateSettings, externalFaviconsReady, networkAccessReady]);

  return (
    <SettingsContext.Provider value={value}>
      {children}
    </SettingsContext.Provider>
  );
}