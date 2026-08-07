import { useState, useEffect, useCallback, useRef } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { X, Monitor, Palette, Shield, Database, Trash2 } from 'lucide-react';
import { useAppState } from '@/contexts/AppStateContext';
import { useTranslation } from '@/contexts/LanguageContext';
import ChangeMasterPasswordModal from './ChangeMasterPasswordModal';
import ImportModal from './ImportModal';
import Hardware2FaModal from './Hardware2FaModal';
import { useBackend } from '@/lib/useBackend';
import { isTauri, type BiometricInfo } from '@/lib/backend';
import { ActionTooltip } from './ui/tooltip';

import { GeneralTab } from './settings/GeneralTab';
import { AppearanceTab } from './settings/AppearanceTab';
import { SecurityTab } from './settings/SecurityTab';
import { BackupTab } from './settings/BackupTab';
import { TrashTab } from './settings/TrashTab';

type Tab = 'general' | 'appearance' | 'security' | 'backup' | 'trash';

const TABS: { id: Tab; labelKey: string; icon: React.ReactNode }[] = [
  { id: 'general', labelKey: 'settings.tab_general', icon: <Monitor size={14} /> },
  { id: 'appearance', labelKey: 'settings.tab_appearance', icon: <Palette size={14} /> },
  { id: 'security', labelKey: 'settings.tab_security', icon: <Shield size={14} /> },
  { id: 'backup', labelKey: 'settings.tab_backup', icon: <Database size={14} /> },
  { id: 'trash', labelKey: 'settings.tab_trash', icon: <Trash2 size={14} /> },
];

export default function SettingsPanel() {
  const { settingsOpen, setSettingsOpen, addToast, currentVault } = useAppState();
  const { t } = useTranslation();
  const { backend } = useBackend();

  const [activeTab, setActiveTab] = useState<Tab>('general');
  const [showChangePassword, setShowChangePassword] = useState(false);
  const [showImportModal, setShowImportModal] = useState(false);

  const [launchOnStartup, setLaunchOnStartup] = useState(false);
  const tabsRef = useRef<HTMLDivElement>(null);

  // Biometric state
  const [bioActive, setBioActive] = useState(false);
  const [bioInfo, setBioInfo] = useState<BiometricInfo | null>(null);

  // Hardware 2FA state & modal
  const [hwActive, setHwActive] = useState(false);
  const [showHwModal, setShowHwModal] = useState(false);
  const [hwModalMode, setHwModalMode] = useState<'enroll' | 'test'>('enroll');

  // Primary Unlock State
  const [primaryUnlock, setPrimaryUnlock] = useState<'master_password' | 'biometric' | 'hardware_2fa'>('master_password');

  useEffect(() => {
    if (backend && currentVault?.path) {
      backend.isBiometricEnabled(currentVault.path).then(setBioActive);
      backend.checkBiometricAvailable().then(setBioInfo);
      backend.isHardware2FaEnabled(currentVault.path).then(setHwActive);
    }
  }, [backend, currentVault]);

  useEffect(() => {
    if (currentVault?.path) {
      const saved = localStorage.getItem(`yntra-vault-primary-unlock-${currentVault.path}`);
      if (saved === 'biometric' || saved === 'hardware_2fa' || saved === 'master_password') {
        setPrimaryUnlock(saved);
      } else {
        setPrimaryUnlock('master_password');
      }
    }
  }, [currentVault]);

  const handleSelectPrimaryUnlock = (method: 'master_password' | 'biometric' | 'hardware_2fa') => {
    if (!currentVault?.path) return;
    if (method === 'biometric' && !bioActive) {
      addToast({ message: 'Enable Windows Hello / Biometrics first', type: 'error' });
      return;
    }
    if (method === 'hardware_2fa' && !hwActive) {
      addToast({ message: 'Enroll a YubiKey / Hardware 2FA key first', type: 'error' });
      return;
    }
    setPrimaryUnlock(method);
    localStorage.setItem(`yntra-vault-primary-unlock-${currentVault.path}`, method);
    addToast({ message: `Primary login method set to ${method.replace('_', ' ')}`, type: 'success' });
  };

  const handleTabsWheel = useCallback((e: React.WheelEvent<HTMLDivElement>) => {
    if (tabsRef.current) {
      tabsRef.current.scrollLeft += e.deltaY;
    }
  }, []);

  const handleToggleLaunch = async (val: boolean) => {
    if (!isTauri()) {
      addToast({ message: 'Autostart feature unavailable in web mode', type: 'error' });
      return;
    }
    try {
      const pluginName = '@tauri-apps/plugin-autostart';
      // @ts-ignore
      const { enable, disable } = await import(/* @vite-ignore */ pluginName);
      if (val) {
        await enable();
      } else {
        await disable();
      }
      setLaunchOnStartup(val);
      addToast({ message: val ? 'Launch on startup enabled' : 'Launch on startup disabled', type: 'info' });
    } catch (e) {
      console.error('Autostart toggle failed:', e);
      addToast({ message: 'Autostart feature unavailable in dev mode', type: 'error' });
    }
  };

  useEffect(() => {
    const checkLaunch = async () => {
      if (!isTauri()) return;
      try {
        const pluginName = '@tauri-apps/plugin-autostart';
        // @ts-ignore
        const { isEnabled } = await import(/* @vite-ignore */ pluginName);
        const enabled = await isEnabled();
        setLaunchOnStartup(enabled);
      } catch (e) {
        console.error('Autostart check failed:', e);
      }
    };
    if (settingsOpen) {
      checkLaunch();
    }
  }, [settingsOpen]);

  const handleToggleBiometric = async () => {
    if (!backend) return;
    try {
      if (bioActive) {
        await backend.disableBiometric();
        setBioActive(false);
        addToast({ message: 'Biometric unlock disabled', type: 'success' });
      } else {
        await backend.enableBiometric();
        setBioActive(true);
        addToast({ message: 'Biometric unlock enabled', type: 'success' });
      }
    } catch (err: any) {
      addToast({ message: err.toString() || 'Biometric toggle failed', type: 'error' });
    }
  };

  const handleDisableHw = async () => {
    if (!backend) return;
    if (!confirm('Are you sure you want to disable Hardware 2FA for this vault?')) return;
    try {
      await backend.disableHardware2Fa();
      setHwActive(false);
      addToast({ message: 'Hardware 2FA disabled', type: 'success' });
    } catch (err: any) {
      addToast({ message: err.toString() || 'Failed to disable Hardware 2FA', type: 'error' });
    }
  };

  return (
    <AnimatePresence>
      {settingsOpen && (
        <>
          <motion.div
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            onClick={() => setSettingsOpen(false)}
            className="fixed inset-0 z-40 bg-black/40 backdrop-blur-[2px]"
          />

          <motion.div
            initial={{ x: '100%' }}
            animate={{ x: 0 }}
            exit={{ x: '100%' }}
            transition={{ duration: 0.2, ease: [0.16, 1, 0.3, 1] }}
            className="fixed right-0 top-0 z-50 flex h-full w-[480px] flex-col border-l border-[var(--border)] bg-[var(--bg-base)]"
          >
            {/* Header */}
            <div className="flex h-12 shrink-0 items-center justify-between border-b border-[var(--border-subtle)] px-4">
              <h2 className="text-[16px] font-semibold text-[var(--text-primary)]">{t('settings.title')}</h2>
              <ActionTooltip content={t('common.close')} side="left">
                <button
                  onClick={() => setSettingsOpen(false)}
                  className="inline-flex items-center justify-center rounded-[3px] p-1 text-[var(--text-tertiary)] transition-colors hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"
                >
                  <X size={18} />
                </button>
              </ActionTooltip>
            </div>

            {/* Tabs */}
            <div
              ref={tabsRef}
              onWheel={handleTabsWheel}
              className="flex h-10 shrink-0 items-center gap-0 border-b border-[var(--border-subtle)] px-4 overflow-x-auto no-scrollbar"
            >
              {TABS.map((tab) => (
                <button
                  key={tab.id}
                  onClick={() => setActiveTab(tab.id)}
                  className={`flex h-full shrink-0 items-center gap-1.5 px-3 text-[12px] font-medium whitespace-nowrap transition-colors ${
                    activeTab === tab.id
                      ? 'border-b-2 border-[var(--text-primary)] text-[var(--text-primary)]'
                      : 'text-[var(--text-secondary)] hover:text-[var(--text-primary)]'
                  }`}
                >
                  {tab.icon}
                  {t(tab.labelKey)}
                </button>
              ))}
            </div>

            {/* Tab Content */}
            <motion.div
              key={activeTab}
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              transition={{ duration: 0.15, delay: 0.1 }}
              className="flex-1 overflow-y-auto p-4"
            >
              {activeTab === 'general' && (
                <GeneralTab
                  launchOnStartup={launchOnStartup}
                  onToggleLaunch={handleToggleLaunch}
                />
              )}

              {activeTab === 'appearance' && (
                <AppearanceTab />
              )}

              {activeTab === 'security' && (
                <SecurityTab
                  bioActive={bioActive}
                  bioInfo={bioInfo}
                  onToggleBiometric={handleToggleBiometric}
                  hwActive={hwActive}
                  onOpenHwModal={(mode) => {
                    setHwModalMode(mode);
                    setShowHwModal(true);
                  }}
                  onDisableHw={handleDisableHw}
                  primaryUnlock={primaryUnlock}
                  onSelectPrimaryUnlock={handleSelectPrimaryUnlock}
                  onOpenChangePassword={() => setShowChangePassword(true)}
                />
              )}

              {activeTab === 'backup' && (
                <BackupTab
                  onOpenImportModal={() => setShowImportModal(true)}
                />
              )}

              {activeTab === 'trash' && (
                <TrashTab />
              )}
            </motion.div>

            {/* Modals */}
            <ChangeMasterPasswordModal
              open={showChangePassword}
              onClose={() => setShowChangePassword(false)}
            />

            <ImportModal
              isOpen={showImportModal}
              onClose={() => setShowImportModal(false)}
            />

            <Hardware2FaModal
              open={showHwModal}
              onClose={() => setShowHwModal(false)}
              mode={hwModalMode}
              onSuccess={() => {
                setHwActive(true);
                addToast({ message: 'Hardware 2FA updated!', type: 'success' });
              }}
            />
          </motion.div>
        </>
      )}
    </AnimatePresence>
  );
}
