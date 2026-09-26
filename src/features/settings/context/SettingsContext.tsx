import { appMetadata } from '@/lib/appMetadata';
import React, { createContext, useContext, useState, useCallback, useEffect, useMemo, useRef } from 'react';
import type { AppSettings } from '@/types';
import { DEFAULT_KEYBINDS } from '@/lib/keybinds';
import { isTauri, getBackend } from '@/lib/backend';

export const DEFAULT_SETTINGS: AppSettings = {
  theme: 'system',
  language: 'en',
  sidebarWidth: 220,
  passwordListWidth: 320,
  fontSize: 13,
  density: 'normal',
  autoLockMinutes: 15,
  clipboardClearSeconds: 30,
  minimizeToTray: true,
  launchOnStartup: false,
  disableSkeletonDelays: false,
  autoBreachCheck: false,
  showBreachInList: true,
  autotypeCharDelayMs: 15,
  autotypeFieldDelayMs: 300,
  autotypeSettleDelayMs: 3000,
  autotypeLaunchBrowser: true,
  tagSortOrder: 'custom',
  showTagCounts: true,
  entrySortOrder: 'updated',
  groupByDate: true,
  keybinds: DEFAULT_KEYBINDS,
  forceMobileView: false,
  windowCaptureProtection: true,
  lockOnFocusLoss: false,
  lockOnSystemLock: true,
  webdavEnabled: false,
  webdavUrl: '',
  webdavUser: '',
  webdavAutoSync: false,
  p2pAddr: '0.0.0.0:5322',
  p2pAutoListen: false,
  p2pAutoSyncWifi: false,
  p2pAutoSyncIntervalMinutes: 5,
  externalFaviconsEnabled: true,
  autoCheckUpdates: false,
  operationMode: 'standard',
};

export interface SettingsContextType {
  settings: AppSettings;
  updateSettings: (partial: Partial<AppSettings>) => void;
  externalFaviconsReady: boolean;
}

const SettingsContext = createContext<SettingsContextType | undefined>(undefined);

export function SettingsProvider({ children }: { children: React.ReactNode }) {
  const [externalFaviconsReady, setExternalFaviconsReady] = useState(false);
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
    if (partial.externalFaviconsEnabled !== undefined && partial.externalFaviconsEnabled !== settings.externalFaviconsEnabled) {
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
  }, [settings.externalFaviconsEnabled]);

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
    const isEnabled = settings.externalFaviconsEnabled === true;
    let cancelled = false;
    if (!isEnabled) window.dispatchEvent(new CustomEvent('yntra-favicons-cleared'));
    // Serialize toggles and wait for the native gate before any icon request.
    faviconSync.current = faviconSync.current.catch(() => {}).then(async () => {
      if (cancelled) return;
      if (isTauri()) await (await getBackend()).setExternalFaviconsEnabled(isEnabled);
      if (!cancelled) {
        setExternalFaviconsReady(isEnabled);
        if (isEnabled) window.dispatchEvent(new CustomEvent('yntra-favicons-reset'));
      }
    }).catch(err => console.error('Failed to sync externalFaviconsEnabled setting:', err));
    return () => { cancelled = true; };
  }, [settings.externalFaviconsEnabled]);

  // Apply font size & density globally to document root
  useEffect(() => {
    if (typeof document !== 'undefined') {
      if (settings.fontSize) {
        document.documentElement.style.fontSize = `${settings.fontSize}px`;
      }
      document.documentElement.setAttribute('data-density', settings.density || 'normal');
    }
  }, [settings.fontSize, settings.density]);

  const value = useMemo(() => ({ settings, updateSettings, externalFaviconsReady }), [settings, updateSettings, externalFaviconsReady]);

  return (
    <SettingsContext.Provider value={value}>
      {children}
    </SettingsContext.Provider>
  );
}

export function useSettings(): SettingsContextType {
  const ctx = useContext(SettingsContext);
  if (!ctx) throw new Error('useSettings must be used within SettingsProvider');
  return ctx;
}
