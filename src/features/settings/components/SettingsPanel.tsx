import { useCapabilities } from '@/lib/platform';
import { useState, useEffect, useCallback, useRef } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { X, Monitor, Palette, Keyboard, Shield, Database, Trash2 } from 'lucide-react';
import { useAuth, ChangeMasterPasswordModal, Hardware2FaModal } from '@/features/auth';
import { useUi } from '@/contexts/UiContext';
import { useToast } from '@/contexts/ToastContext';
import { useTranslation } from '@/contexts/LanguageContext';
import { ImportModal } from '@/features/sync';
import { useBackend } from '@/lib/useBackend';
import { useEntries } from '@/features/entries';
import { isTauri, type BiometricInfo } from '@/lib/backend';
import { ActionTooltip } from '@/components/ui/tooltip';

import { GeneralTab } from './GeneralTab';
import { AppearanceTab } from './AppearanceTab';
import { KeybindsTab } from './KeybindsTab';
import { SecurityTab } from './SecurityTab';
import { BackupTab } from './BackupTab';
import { TrashTab } from './TrashTab';

type Tab = 'general' | 'appearance' | 'keybinds' | 'security' | 'backup' | 'trash';

const TABS: { id: Tab; labelKey: string; icon: React.ReactNode }[] = [
  { id: 'general', labelKey: 'settings.tab_general', icon: <Monitor size={14} /> },
  { id: 'appearance', labelKey: 'settings.tab_appearance', icon: <Palette size={14} /> },
  { id: 'keybinds', labelKey: 'settings.tab_keybinds', icon: <Keyboard size={14} /> },
  { id: 'security', labelKey: 'settings.tab_security', icon: <Shield size={14} /> },
  { id: 'backup', labelKey: 'settings.tab_backup', icon: <Database size={14} /> },
  { id: 'trash', labelKey: 'settings.tab_trash', icon: <Trash2 size={14} /> },
];

export function SettingsPanel() {
  const { currentVault } = useAuth();
  const { settingsOpen, setSettingsOpen, setIsEditing, setFilterCategory, openEditModal } = useUi();
  const { entries, selectEntryById } = useEntries();
  const { addToast } = useToast();
  const capabilities = useCapabilities();
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
  const [isTogglingBio, setIsTogglingBio] = useState(false);

  // Hardware 2FA state & modal
  const [hwLegacy, setHwLegacy] = useState(false);
  const [hwActive, setHwActive] = useState(false);
  const [showHwModal, setShowHwModal] = useState(false);
  const [hwModalMode, setHwModalMode] = useState<'enroll' | 'test'>('enroll');

  useEffect(() => {
    if (backend && currentVault?.path) {
      backend.isBiometricEnabled(currentVault.path).then(setBioActive);
      backend.checkBiometricAvailable().then(setBioInfo);
      backend.isHardware2FaEnabled(currentVault.path).then(setHwActive);
      backend.getHardware2FaChallenge(currentVault.path).then(info => setHwLegacy(Boolean(info && !info.payload_key_bound))).catch(() => {});
    }
  }, [backend, currentVault]);

  const handleTabsWheel = useCallback((e: React.WheelEvent<HTMLDivElement>) => {
    if (tabsRef.current) {
      tabsRef.current.scrollLeft += e.deltaY;
    }
  }, []);

  const handleToggleLaunch = async (val: boolean) => {
    if (!backend || !isTauri()) {
      addToast({ message: t('toast.autostart_unavailable_web'), type: 'error' });
      return;
    }
    try {
      if (val) {
        await backend.enableAutostart();
      } else {
        await backend.disableAutostart();
      }
      setLaunchOnStartup(val);
      addToast({ message: val ? t('toast.autostart_enabled') : t('toast.autostart_disabled'), type: 'info' });
    } catch (e) {
      console.error('Autostart toggle failed:', e);
      addToast({ message: t('toast.autostart_unavailable_dev'), type: 'error' });
    }
  };

  useEffect(() => {
    const checkLaunch = async () => {
      if (!backend || !isTauri()) return;
      try {
        const enabled = await backend.isAutostartEnabled();
        setLaunchOnStartup(enabled);
      } catch (e) {
        console.error('Autostart check failed:', e);
      }
    };
    if (settingsOpen) {
      checkLaunch();
    }
  }, [settingsOpen, backend]);

  const handleToggleBiometric = async () => {
    if (!backend || isTogglingBio) return;
    if (!bioActive && hwActive) {
      addToast({ message: 'Cannot enable Biometric unlock while Hardware 2FA is active.', type: 'error' });
      return;
    }
    setIsTogglingBio(true);
    try {
      if (bioActive) {
        await backend.disableBiometric();
        setBioActive(false);
        addToast({ message: t('toast.biometric_disabled'), type: 'success' });
      } else {
        await backend.enableBiometric();
        setBioActive(true);
        addToast({ message: t('toast.biometric_enabled'), type: 'success' });
      }
    } catch (err: any) {
      const msg = err ? err.toString() : t('toast.biometric_failed');
      if (msg.includes('canceled') || msg.includes('cancelled') || msg.includes('BiometricCanceled')) {
        addToast({ message: 'Biometric verification was canceled', type: 'info' });
      } else {
        addToast({ message: msg, type: 'error' });
      }
    } finally {
      setIsTogglingBio(false);
    }
  };

  const handleDisableHw = async () => {
    if (!backend) return;
    if (!confirm('Are you sure you want to disable Hardware 2FA for this vault?')) return;
    try {
      await backend.disableHardware2Fa();
      setHwActive(false);
      addToast({ message: t('toast.hardware_2fa_disabled'), type: 'success' });
    } catch (err: any) {
      addToast({ message: err ? err.toString() : t('toast.hardware_2fa_disable_failed'), type: 'error' });
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
            className="fixed inset-0 z-40 bg-black/40 backdrop-blur-[2px] select-none"
          />

          <motion.div
            initial={{ x: '100%' }}
            animate={{ x: 0 }}
            exit={{ x: '100%' }}
            transition={{ duration: 0.2, ease: [0.16, 1, 0.3, 1] }}
            className="fixed right-0 top-0 z-50 flex h-full w-full sm:w-[480px] max-w-full flex-col border-l border-[var(--border)] bg-[var(--bg-base)] pt-[env(safe-area-inset-top,0px)] pb-[env(safe-area-inset-bottom,0px)]"
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
              className="flex h-10 shrink-0 items-center gap-0 border-b border-[var(--border-subtle)] px-4 overflow-x-auto no-scrollbar touch-pan-x"
            >
              {TABS.filter(tab => capabilities.desktop || tab.id !== 'keybinds').map((tab) => (
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
              className="flex-1 min-h-0 overflow-y-auto p-4 pb-[calc(5rem+env(safe-area-inset-bottom,0px))] touch-pan-y overscroll-contain"
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

              {activeTab === 'keybinds' && (
                <KeybindsTab />
              )}

              {activeTab === 'security' && (
                <SecurityTab
                  bioActive={bioActive}
                  bioInfo={bioInfo}
                  onToggleBiometric={handleToggleBiometric}
                  isTogglingBio={isTogglingBio}
                  hwActive={hwActive}
                  hwLegacy={hwLegacy}
                  onOpenHwModal={(mode) => {
                    setHwModalMode(mode);
                    setShowHwModal(true);
                  }}
                  onDisableHw={handleDisableHw}
                  onOpenChangePassword={() => setShowChangePassword(true)}
                  onNavigateToEntry={(entryId) => {
                    const entry = entries.find((e) => e.id === entryId);
                    if (entry) {
                      selectEntryById(entryId);
                      setFilterCategory('all');
                      setIsEditing(false);
                      setSettingsOpen(false);
                      openEditModal(entry);
                    }
                  }}
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
                setHwLegacy(false);
                setBioActive(false);
                addToast({ message: 'Hardware 2FA configured!', type: 'success' });
              }}
            />
          </motion.div>
        </>
      )}
    </AnimatePresence>
  );
}

export default SettingsPanel;
