import { DEFAULT_KEYBINDS } from '@/lib/keybinds';
import type { AppSettings } from '@/types';
import { createContext,useContext } from 'react';


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
  networkAccessReady: boolean;
}

export const SettingsContext = createContext<SettingsContextType | undefined>(undefined);


export function useSettings(): SettingsContextType {
  const ctx = useContext(SettingsContext);
  if (!ctx) throw new Error('useSettings must be used within SettingsProvider');
  return ctx;
}