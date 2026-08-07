import { useState, useEffect, useCallback, useRef } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { X, Monitor, Sun, Moon, Palette, Database, Shield, Trash2, RotateCcw, Trash } from 'lucide-react';
import { useAppState } from '@/contexts/AppStateContext';
import { useTheme } from '@/contexts/ThemeContext';
import { useTranslation } from '@/contexts/LanguageContext';
import { LanguageCombobox } from './ui/LanguageCombobox';
import { SecurityDashboard } from './SecurityDashboard';
import ChangeMasterPasswordModal from './ChangeMasterPasswordModal';
import { useBackend } from '@/lib/useBackend';
import { isTauri, type TrashedEntryPreview } from '@/lib/backend';
import { ActionTooltip } from './ui/tooltip';

type Tab = 'general' | 'appearance' | 'security' | 'backup' | 'trash';

const TABS: { id: Tab; labelKey: string; icon: React.ReactNode }[] = [
  { id: 'general', labelKey: 'settings.tab_general', icon: <Monitor size={14} /> },
  { id: 'appearance', labelKey: 'settings.tab_appearance', icon: <Palette size={14} /> },
  { id: 'security', labelKey: 'settings.tab_security', icon: <Shield size={14} /> },
  { id: 'backup', labelKey: 'settings.tab_backup', icon: <Database size={14} /> },
  { id: 'trash', labelKey: 'settings.tab_trash', icon: <Trash2 size={14} /> },
];

const AUTO_LOCK_OPTIONS = [
  { value: 1, labelKey: 'time.1_min' },
  { value: 5, labelKey: 'time.5_min' },
  { value: 15, labelKey: 'time.15_min' },
  { value: 30, labelKey: 'time.30_min' },
  { value: 0, labelKey: 'time.never' },
];

const CLIPBOARD_OPTIONS = [
  { value: 10, labelKey: 'time.10_sec' },
  { value: 30, labelKey: 'time.30_sec' },
  { value: 60, labelKey: 'time.1_min' },
  { value: 300, labelKey: 'time.5_min' },
  { value: 0, labelKey: 'time.never' },
];

export default function SettingsPanel() {
  const { settingsOpen, setSettingsOpen, settings, updateSettings, refreshEntries, addToast, currentVault, setIsLocked, setCurrentVault } = useAppState();
  const { theme, setTheme } = useTheme();
  const { t } = useTranslation();
  const [activeTab, setActiveTab] = useState<Tab>('general');
  const [showChangePassword, setShowChangePassword] = useState(false);

  const [launchOnStartup, setLaunchOnStartup] = useState(false);

  // WebDAV settings state
  const [webdavEnabled, setWebdavEnabled] = useState(false);
  const [autoSyncOnSave, setAutoSyncOnSave] = useState(false);
  const [webdavUrl, setWebdavUrl] = useState('');
  const [webdavUser, setWebdavUser] = useState('');
  const [webdavPass, setWebdavPass] = useState('');
  const [isTestingWebdav, setIsTestingWebdav] = useState(false);
  const [isSyncingWebdav, setIsSyncingWebdav] = useState(false);

  // P2P settings state
  const [p2pAddr, setP2pAddr] = useState('127.0.0.1:5322');
  const [isSyncingP2P, setIsSyncingP2P] = useState(false);

  // Shamir settings state
  const [shamirPass, setShamirPass] = useState('');
  const [shares, setShares] = useState<string[]>([]);
  const [shareA, setShareA] = useState('');
  const [shareB, setShareB] = useState('');
  const [reconstructedHash, setReconstructedHash] = useState('');

  const { backend } = useBackend();
  const tabsRef = useRef<HTMLDivElement>(null);

  const handleTabsWheel = useCallback((e: React.WheelEvent<HTMLDivElement>) => {
    if (tabsRef.current && e.deltaY !== 0) {
      tabsRef.current.scrollLeft += e.deltaY;
    }
  }, []);
  const [trashItems, setTrashItems] = useState<TrashedEntryPreview[]>([]);
  const [loadingTrash, setLoadingTrash] = useState(false);

  // Query autostart status on panel load
  useEffect(() => {
    if (settingsOpen && backend) {
      backend.isAutostartEnabled()
        .then(setLaunchOnStartup)
        .catch((err) => console.error('Failed to query autostart status:', err));
    }
  }, [settingsOpen, backend]);

  const handleToggleLaunch = async (v: boolean) => {
    if (!backend) return;
    try {
      if (v) {
        await backend.enableAutostart();
      } else {
        await backend.disableAutostart();
      }
      setLaunchOnStartup(v);
      updateSettings({ launchOnStartup: v });
      addToast({ message: `Autostart ${v ? 'enabled' : 'disabled'}`, type: 'success' });
    } catch (err) {
      addToast({ message: `Autostart toggle failed: ${err}`, type: 'error' });
    }
  };

  const fetchTrash = useCallback(async () => {
    if (!backend) return;
    setLoadingTrash(true);
    try {
      const items = await backend.listTrash();
      setTrashItems(items);
    } catch (e) {
      console.error('Failed to fetch trash:', e);
    } finally {
      setLoadingTrash(false);
    }
  }, [backend]);

  useEffect(() => {
    if (activeTab === 'trash' && settingsOpen) {
      fetchTrash();
    }
  }, [activeTab, settingsOpen, fetchTrash]);

  const handleRestore = async (id: string) => {
    if (!backend) return;
    try {
      await backend.restoreFromTrash(id);
      addToast({ message: 'Entry restored', type: 'success' });
      await fetchTrash();
      await refreshEntries();
    } catch (e) {
      addToast({ message: `Failed to restore: ${e}`, type: 'error' });
    }
  };

  const handlePermanentDelete = async (id: string) => {
    if (!backend) return;
    try {
      await backend.permanentDelete(id);
      addToast({ message: 'Entry permanently deleted', type: 'info' });
      await fetchTrash();
    } catch (e) {
      addToast({ message: `Failed to delete permanently: ${e}`, type: 'error' });
    }
  };

  const handleEmptyTrash = async () => {
    if (!backend || trashItems.length === 0) return;
    if (!confirm('Are you sure you want to permanently delete all items in trash? This cannot be undone.')) return;
    try {
      await Promise.all(trashItems.map(item => backend.permanentDelete(item.id)));
      addToast({ message: 'Trash emptied', type: 'info' });
      await fetchTrash();
    } catch (e) {
      addToast({ message: `Failed to empty trash: ${e}`, type: 'error' });
    }
  };

  return (
    <AnimatePresence>
      {settingsOpen && (
        <>
          {/* Backdrop */}
          <motion.div
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            transition={{ duration: 0.2 }}
            className="fixed inset-0 z-40 bg-black/30"
            onClick={() => setSettingsOpen(false)}
          />

          {/* Panel */}
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

            {/* Content */}
            <motion.div
              key={activeTab}
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              transition={{ duration: 0.15, delay: 0.1 }}
              className="flex-1 overflow-y-auto p-4"
            >
              {activeTab === 'general' && (
                <div className="flex flex-col gap-6">
                  {currentVault && (
                    <SettingSection label={t('settings.active_vault')}>
                      <div className="flex flex-col gap-2.5 rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] p-3">
                        <div className="flex flex-col gap-0.5">
                          <span className="text-[10px] font-bold uppercase tracking-wider text-[var(--text-tertiary)]">{t('settings.vault_name')}</span>
                          <span className="text-[13px] font-medium text-[var(--text-primary)]">{currentVault.name}</span>
                        </div>
                        <div className="flex flex-col gap-0.5">
                          <span className="text-[10px] font-bold uppercase tracking-wider text-[var(--text-tertiary)]">{t('settings.file_location')}</span>
                          <span className="font-mono text-[11px] text-[var(--text-secondary)] break-all">{currentVault.path}</span>
                        </div>
                        {isTauri() && (
                          <button
                            type="button"
                            onClick={() => {
                              backend?.showInExplorer(currentVault.path).catch(err => {
                                addToast({ message: `Failed to open explorer: ${err}`, type: 'error' });
                              });
                            }}
                            className="mt-1 h-7 self-start rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] px-2.5 text-[11px] font-medium text-[var(--text-primary)] transition-colors hover:bg-[var(--bg-hover)]"
                          >
                            {t('settings.show_in_explorer')}
                          </button>
                        )}
                      </div>
                    </SettingSection>
                  )}

                  <SettingSection label={t('settings.language_label')}>
                    <p className="mb-2.5 text-[12px] text-[var(--text-secondary)]">
                      {t('settings.language_desc')}
                    </p>
                    <LanguageCombobox />
                  </SettingSection>

                  <SettingSection label={t('settings.autolock_label')}>
                    <p className="mb-2 text-[12px] text-[var(--text-secondary)]">
                      {t('settings.autolock_desc')}
                    </p>
                    <select
                      value={settings.autoLockMinutes}
                      onChange={(e) => updateSettings({ autoLockMinutes: Number(e.target.value) })}
                      className="h-8 w-full rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] px-2.5 text-[13px] text-[var(--text-primary)] outline-none focus:border-[var(--border-focus)]"
                    >
                      {AUTO_LOCK_OPTIONS.map((o) => (
                        <option key={o.value} value={o.value}>
                          {t(o.labelKey)}
                        </option>
                      ))}
                    </select>
                  </SettingSection>

                  <SettingSection label={t('settings.clipboard_label')}>
                    <p className="mb-2 text-[12px] text-[var(--text-secondary)]">
                      {t('settings.clipboard_desc')}
                    </p>
                    <select
                      value={settings.clipboardClearSeconds}
                      onChange={(e) =>
                        updateSettings({ clipboardClearSeconds: Number(e.target.value) })
                      }
                      className="h-8 w-full rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] px-2.5 text-[13px] text-[var(--text-primary)] outline-none focus:border-[var(--border-focus)]"
                    >
                      {CLIPBOARD_OPTIONS.map((o) => (
                        <option key={o.value} value={o.value}>
                          {t(o.labelKey)}
                        </option>
                      ))}
                    </select>
                  </SettingSection>

                  <SettingSection label={t('settings.autotype_title')}>
                    <div className="flex flex-col gap-4">
                      <div>
                        <div className="flex justify-between items-center mb-1">
                          <span className="text-[12px] text-[var(--text-secondary)]">{t('settings.char_delay')}</span>
                          <span className="text-[11px] font-mono text-[var(--text-primary)]">{(settings.autotypeCharDelayMs ?? 15)} ms</span>
                        </div>
                        <input
                          type="range"
                          min={5}
                          max={100}
                          step={5}
                          value={settings.autotypeCharDelayMs ?? 15}
                          onChange={(e) => updateSettings({ autotypeCharDelayMs: Number(e.target.value) })}
                          className="h-1 w-full appearance-none rounded-full bg-[var(--border)] outline-none [&::-webkit-slider-thumb]:h-3 [&::-webkit-slider-thumb]:w-3 [&::-webkit-slider-thumb]:appearance-none [&::-webkit-slider-thumb]:rounded-full [&::-webkit-slider-thumb]:bg-[var(--text-primary)]"
                        />
                      </div>
                      <div>
                        <div className="flex justify-between items-center mb-1">
                          <span className="text-[12px] text-[var(--text-secondary)]">{t('settings.field_delay')}</span>
                          <span className="text-[11px] font-mono text-[var(--text-primary)]">{(settings.autotypeFieldDelayMs ?? 300)} ms</span>
                        </div>
                        <input
                          type="range"
                          min={100}
                          max={2000}
                          step={100}
                          value={settings.autotypeFieldDelayMs ?? 300}
                          onChange={(e) => updateSettings({ autotypeFieldDelayMs: Number(e.target.value) })}
                          className="h-1 w-full appearance-none rounded-full bg-[var(--border)] outline-none [&::-webkit-slider-thumb]:h-3 [&::-webkit-slider-thumb]:w-3 [&::-webkit-slider-thumb]:appearance-none [&::-webkit-slider-thumb]:rounded-full [&::-webkit-slider-thumb]:bg-[var(--text-primary)]"
                        />
                      </div>
                      <div>
                        <div className="flex justify-between items-center mb-1">
                          <span className="text-[12px] text-[var(--text-secondary)]">{t('settings.settle_delay')}</span>
                          <span className="text-[11px] font-mono text-[var(--text-primary)]">{((settings.autotypeSettleDelayMs ?? 3000) / 1000).toFixed(1)} s</span>
                        </div>
                        <input
                          type="range"
                          min={500}
                          max={5000}
                          step={500}
                          value={settings.autotypeSettleDelayMs ?? 3000}
                          onChange={(e) => updateSettings({ autotypeSettleDelayMs: Number(e.target.value) })}
                          className="h-1 w-full appearance-none rounded-full bg-[var(--border)] outline-none [&::-webkit-slider-thumb]:h-3 [&::-webkit-slider-thumb]:w-3 [&::-webkit-slider-thumb]:appearance-none [&::-webkit-slider-thumb]:rounded-full [&::-webkit-slider-thumb]:bg-[var(--text-primary)]"
                        />
                      </div>
                    </div>
                  </SettingSection>

                  <SettingRow
                    label={t('settings.autotype_launch_browser')}
                    description={t('settings.autotype_launch_browser_desc')}
                  >
                    <Toggle
                      checked={settings.autotypeLaunchBrowser !== false}
                      onChange={(v) => updateSettings({ autotypeLaunchBrowser: v })}
                    />
                  </SettingRow>

                  <SettingRow
                    label={t('settings.minimize_to_tray')}
                    description={t('settings.minimize_to_tray_desc')}
                  >
                    <Toggle
                      checked={settings.minimizeToTray}
                      onChange={(v) => updateSettings({ minimizeToTray: v })}
                    />
                  </SettingRow>

                  <SettingRow label={t('settings.autostart_label')} description={t('settings.autostart_desc')}>
                    <Toggle
                      checked={launchOnStartup}
                      onChange={handleToggleLaunch}
                    />
                  </SettingRow>

                  <SettingRow
                    label={t('settings.disable_skeleton_delays')}
                    description={t('settings.disable_skeleton_delays_desc')}
                  >
                    <Toggle
                      checked={settings.disableSkeletonDelays}
                      onChange={(v) => updateSettings({ disableSkeletonDelays: v })}
                    />
                  </SettingRow>

                  <SettingRow
                    label={t('settings.breach_label')}
                    description={t('settings.breach_desc')}
                  >
                    <Toggle
                      checked={settings.autoBreachCheck}
                      onChange={(v) => updateSettings({ autoBreachCheck: v })}
                    />
                  </SettingRow>

                  <SettingRow
                    label={t('settings.show_breach_in_list')}
                    description={t('settings.show_breach_in_list_desc')}
                  >
                    <Toggle
                      checked={settings.showBreachInList}
                      onChange={(v) => updateSettings({ showBreachInList: v })}
                    />
                  </SettingRow>
                </div>
              )}

              {activeTab === 'appearance' && (
                <div className="flex flex-col gap-6">
                  <SettingSection label={t('settings.theme_label')}>
                    <div className="flex gap-2">
                      {([
                        { value: 'light' as const, label: t('settings.theme_light'), icon: <Sun size={20} /> },
                        { value: 'dark' as const, label: t('settings.theme_dark'), icon: <Moon size={20} /> },
                        { value: 'system' as const, label: t('settings.theme_system'), icon: <Monitor size={20} /> },
                      ]).map((t) => (
                        <button
                          key={t.value}
                          onClick={() => setTheme(t.value)}
                          className={`flex h-[72px] w-[100px] flex-col items-center justify-center gap-1.5 rounded-[3px] border text-[12px] font-medium transition-colors ${
                            theme === t.value
                              ? 'border-[var(--text-primary)] text-[var(--text-primary)]'
                              : 'border-[var(--border)] text-[var(--text-secondary)] hover:border-[var(--border-focus)]'
                          }`}
                        >
                          {t.icon}
                          {t.label}
                        </button>
                      ))}
                    </div>
                  </SettingSection>

                  <SettingSection label={t('settings.font_size')}>
                    <div className="flex items-center gap-4">
                      <input
                        type="range"
                        min={12}
                        max={16}
                        step={1}
                        value={settings.fontSize}
                        onChange={(e) => updateSettings({ fontSize: Number(e.target.value) })}
                        className="h-1 flex-1 appearance-none rounded-full bg-[var(--border)] outline-none [&::-webkit-slider-thumb]:h-3 [&::-webkit-slider-thumb]:w-3 [&::-webkit-slider-thumb]:appearance-none [&::-webkit-slider-thumb]:rounded-full [&::-webkit-slider-thumb]:bg-[var(--text-primary)]"
                      />
                      <span className="w-10 text-right text-[12px] text-[var(--text-secondary)]">
                        {settings.fontSize}px
                      </span>
                    </div>
                  </SettingSection>

                  <SettingSection label={t('settings.density')}>
                    <p className="mb-2 text-[12px] text-[var(--text-secondary)]">
                      {t('settings.density_desc')}
                    </p>
                    <div className="flex rounded-[3px] border border-[var(--border)]">
                      {(['compact', 'normal', 'comfortable'] as const).map((d) => (
                        <button
                          key={d}
                          onClick={() => updateSettings({ density: d })}
                          className={`flex-1 py-1.5 text-[12px] font-medium capitalize transition-colors ${
                            settings.density === d
                              ? 'bg-[var(--bg-active)] text-[var(--text-primary)]'
                              : 'text-[var(--text-secondary)] hover:text-[var(--text-primary)]'
                          }`}
                        >
                          {d === 'compact' ? t('settings.density_compact') : d === 'normal' ? t('settings.density_normal') : t('settings.density_comfortable')}
                        </button>
                      ))}
                    </div>
                  </SettingSection>

                  <SettingSection label={t('onboarding.rerun_setup')}>
                    <button
                      onClick={() => {
                        localStorage.removeItem('yntra-vault-setup-completed');
                        setIsLocked(true);
                        setCurrentVault(null);
                        window.location.href = '/#/setup';
                      }}
                      className="h-8 rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] px-3 text-[12px] font-medium text-[var(--text-primary)] transition-colors hover:bg-[var(--bg-hover)]"
                    >
                      {t('onboarding.rerun_setup')}
                    </button>
                  </SettingSection>
                </div>
              )}

              {activeTab === 'security' && (
                <div className="flex flex-col gap-6">
                  <SettingSection label={t('settings.master_password')}>
                    <p className="mb-3 text-[12px] text-[var(--text-secondary)]">
                      {t('security.master_password')}
                    </p>
                    <button
                      onClick={() => setShowChangePassword(true)}
                      className="h-8 rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] px-3 text-[13px] font-medium text-[var(--text-primary)] transition-colors hover:bg-[var(--bg-hover)]"
                    >
                      {t('settings.change_password')}
                    </button>
                  </SettingSection>

                  <SettingSection label={t('settings.emergency_recovery')}>
                    <p className="mb-3 text-[12px] text-[var(--text-secondary)]">
                      {t('settings.emergency_recovery_desc')}
                    </p>
                    <div className="flex flex-col gap-2 rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] p-3">
                      <div className="flex gap-2">
                        <input
                          type="password"
                          placeholder={t('settings.verify_master_placeholder')}
                          value={shamirPass}
                          onChange={(e) => setShamirPass(e.target.value)}
                          className="h-8 flex-1 rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] px-2.5 text-[13px] text-[var(--text-primary)] outline-none focus:border-[var(--border-focus)]"
                        />
                        <button
                          onClick={async () => {
                            if (!backend || !shamirPass) return;
                            try {
                              const res = await backend.splitMasterPassword(shamirPass);
                              setShares(res);
                              addToast({ message: 'Recovery shares generated!', type: 'success' });
                            } catch (err) {
                              addToast({ message: `Split failed: ${err}`, type: 'error' });
                            }
                          }}
                          className="h-8 rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] px-3 text-[12px] font-medium text-[var(--text-primary)] transition-colors hover:bg-[var(--bg-hover)]"
                        >
                          {t('settings.split_button')}
                        </button>
                      </div>

                      {shares.length > 0 && (
                        <div className="flex flex-col gap-1.5 mt-2 border-t border-[var(--border-subtle)] pt-2.5">
                          <span className="text-[10px] font-bold uppercase tracking-wider text-[var(--text-tertiary)]">{t('settings.recovery_shares_label')}</span>
                          {shares.map((s, idx) => (
                            <div key={idx} className="flex items-center justify-between gap-2 rounded-[3px] bg-[var(--bg-base)] px-2 py-1">
                              <span className="font-mono text-[10px] text-[var(--text-secondary)] select-all truncate">{s}</span>
                              <button
                                onClick={() => {
                                  navigator.clipboard.writeText(s).catch(() => {});
                                  addToast({ message: `Share ${idx + 1} copied`, type: 'success' });
                                }}
                                className="text-[10px] font-medium text-[var(--text-primary)] hover:underline"
                              >
                                {t('common.copy')}
                              </button>
                            </div>
                          ))}
                        </div>
                      )}
                    </div>

                    <div className="flex flex-col gap-2 rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] p-3 mt-3">
                      <span className="text-[11px] font-medium text-[var(--text-primary)]">{t('settings.reconstruct_hash')}</span>
                      <div className="flex flex-col gap-2">
                        <input
                          type="text"
                          placeholder={t('settings.share1_placeholder')}
                          value={shareA}
                          onChange={(e) => setShareA(e.target.value)}
                          className="h-8 w-full rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] px-2.5 text-[12px] text-[var(--text-primary)] outline-none focus:border-[var(--border-focus)]"
                        />
                        <input
                          type="text"
                          placeholder={t('settings.share2_placeholder')}
                          value={shareB}
                          onChange={(e) => setShareB(e.target.value)}
                          className="h-8 w-full rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] px-2.5 text-[12px] text-[var(--text-primary)] outline-none focus:border-[var(--border-focus)]"
                        />
                        <button
                          onClick={async () => {
                            if (!backend || !shareA || !shareB) return;
                            try {
                              const res = await backend.reconstructMasterPasswordHash(shareA, shareB);
                              setReconstructedHash(res);
                              addToast({ message: 'Hash reconstructed successfully', type: 'success' });
                            } catch (err) {
                              addToast({ message: `Reconstruction failed: ${err}`, type: 'error' });
                            }
                          }}
                          className="h-8 w-full rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] text-[12px] font-medium text-[var(--text-primary)] transition-colors hover:bg-[var(--bg-hover)]"
                        >
                          {t('settings.reconstruct_hash')}
                        </button>
                        {reconstructedHash && (
                          <div className="flex flex-col gap-0.5 mt-1 bg-[var(--bg-base)] p-2 rounded-[3px]">
                            <span className="text-[10px] font-bold uppercase tracking-wider text-[var(--text-tertiary)]">{t('settings.reconstructed_hash_label')}</span>
                            <span className="font-mono text-[10px] text-green-500 break-all select-all">{reconstructedHash}</span>
                          </div>
                        )}
                      </div>
                    </div>
                  </SettingSection>

                  <SettingSection label={t('security.title')}>
                    <SecurityDashboard />
                  </SettingSection>
                </div>
              )}

              {activeTab === 'backup' && (
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

                  {/* Manual Export */}
                  <SettingSection label={t('settings.manual_export')}>
                    <p className="mb-3 text-[12px] text-[var(--text-secondary)]">
                      {t('settings.manual_export_desc')}
                    </p>
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
                          addToast({ message: 'Vault exported successfully!', type: 'success' });
                        } catch (err) {
                          addToast({ message: `Export failed: ${err}`, type: 'error' });
                        }
                      }}
                      className="h-8 rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] px-3 text-[13px] font-medium text-[var(--text-primary)] transition-colors hover:bg-[var(--bg-hover)]"
                    >
                      {t('settings.export_file')}
                    </button>
                  </SettingSection>
                </div>
              )}

              {activeTab === 'trash' && (
                <div className="flex flex-col gap-6">
                  <SettingSection label={t('settings.trash_title')}>
                    <p className="mb-3 text-[12px] text-[var(--text-secondary)]">
                      {t('settings.trash_desc')}
                    </p>
                    {trashItems.length > 0 && (
                      <ActionTooltip content={t('settings.trash_empty')}>
                        <button
                          onClick={handleEmptyTrash}
                          className="mb-4 flex items-center gap-1.5 rounded-[3px] border border-red-500/30 bg-red-500/10 px-3 py-1.5 text-[12px] font-medium text-red-500 transition-colors hover:bg-red-500 hover:text-white"
                        >
                          <Trash size={13} />
                          {t('settings.trash_empty')}
                        </button>
                      </ActionTooltip>
                    )}

                    {loadingTrash ? (
                      <p className="text-[12px] text-[var(--text-tertiary)] py-4">{t('settings.loading_trash')}</p>
                    ) : trashItems.length === 0 ? (
                      <div className="flex flex-col items-center justify-center gap-2 py-8 rounded-[3px] border border-dashed border-[var(--border-subtle)]">
                        <Trash2 size={20} className="text-[var(--text-tertiary)]" />
                        <p className="text-[12px] text-[var(--text-tertiary)]">{t('settings.trash_is_empty')}</p>
                      </div>
                    ) : (
                      <div className="flex flex-col gap-[2px] rounded-[3px] border border-[var(--border-subtle)] overflow-hidden">
                        {trashItems.map((item) => (
                          <div
                            key={item.id}
                            className="flex items-center justify-between bg-[var(--bg-elevated)] p-3 text-left transition-colors hover:bg-[var(--bg-hover)]"
                          >
                            <div className="flex flex-col gap-1 min-w-0">
                              <span className="truncate text-[13px] font-medium text-[var(--text-primary)]">
                                {item.title}
                              </span>
                              <span className="text-[10px] text-[var(--text-secondary)]">
                                Deleted: {new Date(item.deleted_at).toLocaleDateString()} • {item.days_until_permanent} days remaining
                              </span>
                            </div>
                            <div className="flex items-center gap-1 shrink-0">
                              <ActionTooltip content={t('settings.restore_entry')}>
                                <button
                                  type="button"
                                  onClick={() => handleRestore(item.id)}
                                  className="inline-flex h-7 items-center gap-1 rounded-[3px] px-2 text-[11px] font-medium text-[var(--text-secondary)] transition-colors hover:bg-[var(--bg-active)] hover:text-[var(--text-primary)]"
                                >
                                  <RotateCcw size={12} />
                                  {t('settings.restore_entry')}
                                </button>
                              </ActionTooltip>
                              <ActionTooltip content={t('settings.delete_permanently')}>
                                <button
                                  type="button"
                                  onClick={() => handlePermanentDelete(item.id)}
                                  className="inline-flex h-7 items-center gap-1 rounded-[3px] px-2 text-[11px] font-medium text-[var(--destructive)] transition-colors hover:bg-[var(--destructive)]/10"
                                >
                                  <Trash2 size={12} />
                                  {t('settings.delete_permanently')}
                                </button>
                              </ActionTooltip>
                            </div>
                          </div>
                        ))}
                      </div>
                    )}
                  </SettingSection>
                </div>
              )}
            </motion.div>

            <ChangeMasterPasswordModal
              open={showChangePassword}
              onClose={() => setShowChangePassword(false)}
            />
          </motion.div>
        </>
      )}
    </AnimatePresence>
  );
}

function SettingSection({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="border-b border-[var(--border-subtle)] pb-5">
      <h3 className="mb-3 text-[11px] font-semibold uppercase tracking-[0.04em] text-[var(--text-tertiary)]">
        {label}
      </h3>
      {children}
    </div>
  );
}

function SettingRow({
  label,
  description,
  children,
}: {
  label: string;
  description?: string;
  children: React.ReactNode;
}) {
  return (
    <div className="flex items-center justify-between border-b border-[var(--border-subtle)] py-3">
      <div>
        <div className="text-[13px] text-[var(--text-primary)]">{label}</div>
        {description && (
          <div className="mt-0.5 text-[12px] text-[var(--text-secondary)]">{description}</div>
        )}
      </div>
      {children}
    </div>
  );
}

function Toggle({ checked, onChange }: { checked: boolean; onChange: (v: boolean) => void }) {
  return (
    <button
      onClick={() => onChange(!checked)}
      role="switch"
      aria-checked={checked}
      className={`relative h-5 w-9 rounded-full transition-colors ${
        checked ? 'bg-[var(--text-primary)]' : 'bg-[var(--border)]'
      }`}
    >
      <div
        className={`absolute top-0.5 h-4 w-4 rounded-full bg-white transition-transform ${
          checked ? 'translate-x-4.5' : 'translate-x-0.5'
        }`}
        style={{ transform: checked ? 'translateX(18px)' : 'translateX(2px)' }}
      />
    </button>
  );
}



