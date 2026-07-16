import { useState, useEffect, useCallback } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { X, Monitor, Sun, Moon, Palette, Database, Shield, Trash2, RotateCcw, Trash } from 'lucide-react';
import { useAppState } from '@/contexts/AppStateContext';
import { useTheme } from '@/contexts/ThemeContext';
import { SecurityDashboard } from './SecurityDashboard';
import ChangeMasterPasswordModal from './ChangeMasterPasswordModal';
import { useBackend } from '@/lib/useBackend';
import type { TrashedEntryPreview } from '@/lib/backend';

type Tab = 'general' | 'appearance' | 'security' | 'backup' | 'trash';

const TABS: { id: Tab; label: string; icon: React.ReactNode }[] = [
  { id: 'general', label: 'General', icon: <Monitor size={14} /> },
  { id: 'appearance', label: 'Appearance', icon: <Palette size={14} /> },
  { id: 'security', label: 'Security', icon: <Shield size={14} /> },
  { id: 'backup', label: 'Backup', icon: <Database size={14} /> },
  { id: 'trash', label: 'Trash', icon: <Trash2 size={14} /> },
];

const AUTO_LOCK_OPTIONS = [
  { value: 1, label: '1 minute' },
  { value: 5, label: '5 minutes' },
  { value: 15, label: '15 minutes' },
  { value: 30, label: '30 minutes' },
  { value: 0, label: 'Never' },
];

const CLIPBOARD_OPTIONS = [
  { value: 10, label: '10 seconds' },
  { value: 30, label: '30 seconds' },
  { value: 60, label: '1 minute' },
  { value: 300, label: '5 minutes' },
  { value: 0, label: 'Never' },
];

export default function SettingsPanel() {
  const { settingsOpen, setSettingsOpen, settings, updateSettings, refreshEntries, addToast } = useAppState();
  const { theme, setTheme } = useTheme();
  const [activeTab, setActiveTab] = useState<Tab>('general');
  const [showChangePassword, setShowChangePassword] = useState(false);

  const { backend } = useBackend();
  const [trashItems, setTrashItems] = useState<TrashedEntryPreview[]>([]);
  const [loadingTrash, setLoadingTrash] = useState(false);

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
              <h2 className="text-[16px] font-semibold text-[var(--text-primary)]">Settings</h2>
              <button
                onClick={() => setSettingsOpen(false)}
                className="inline-flex items-center justify-center rounded-[3px] p-1 text-[var(--text-tertiary)] transition-colors hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"
              >
                <X size={18} />
              </button>
            </div>

            {/* Tabs */}
            <div className="flex h-10 shrink-0 items-center gap-0 border-b border-[var(--border-subtle)] px-4">
              {TABS.map((tab) => (
                <button
                  key={tab.id}
                  onClick={() => setActiveTab(tab.id)}
                  className={`flex h-full items-center gap-1.5 px-3 text-[12px] font-medium transition-colors ${
                    activeTab === tab.id
                      ? 'border-b-2 border-[var(--text-primary)] text-[var(--text-primary)]'
                      : 'text-[var(--text-secondary)] hover:text-[var(--text-primary)]'
                  }`}
                >
                  {tab.icon}
                  {tab.label}
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
                  <SettingSection label="Auto-Lock">
                    <p className="mb-2 text-[12px] text-[var(--text-secondary)]">
                      Lock the vault after a period of inactivity
                    </p>
                    <select
                      value={settings.autoLockMinutes}
                      onChange={(e) => updateSettings({ autoLockMinutes: Number(e.target.value) })}
                      className="h-8 w-full rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] px-2.5 text-[13px] text-[var(--text-primary)] outline-none focus:border-[var(--border-focus)]"
                    >
                      {AUTO_LOCK_OPTIONS.map((o) => (
                        <option key={o.value} value={o.value}>
                          {o.label}
                        </option>
                      ))}
                    </select>
                  </SettingSection>

                  <SettingSection label="Clipboard">
                    <p className="mb-2 text-[12px] text-[var(--text-secondary)]">
                      Clear copied passwords from clipboard after
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
                          {o.label}
                        </option>
                      ))}
                    </select>
                  </SettingSection>

                  <SettingRow
                    label="Minimize to system tray"
                    description="Keep running in background"
                  >
                    <Toggle
                      checked={settings.minimizeToTray}
                      onChange={(v) => updateSettings({ minimizeToTray: v })}
                    />
                  </SettingRow>

                  <SettingRow label="Start on system login">
                    <Toggle
                      checked={settings.launchOnStartup}
                      onChange={(v) => updateSettings({ launchOnStartup: v })}
                    />
                  </SettingRow>

                  <SettingRow
                    label="Disable loading delay"
                    description="Instantly render data without minimum skeleton display time"
                  >
                    <Toggle
                      checked={settings.disableSkeletonDelays}
                      onChange={(v) => updateSettings({ disableSkeletonDelays: v })}
                    />
                  </SettingRow>

                  <SettingRow
                    label="Auto-check password breaches"
                    description="Automatically verify password safety on the web check database"
                  >
                    <Toggle
                      checked={settings.autoBreachCheck}
                      onChange={(v) => updateSettings({ autoBreachCheck: v })}
                    />
                  </SettingRow>

                  <SettingRow
                    label="Show breach alerts in list"
                    description="Display warning badges next to compromised items in the sidebar list"
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
                  <SettingSection label="Theme">
                    <div className="flex gap-2">
                      {([
                        { value: 'light' as const, label: 'Light', icon: <Sun size={20} /> },
                        { value: 'dark' as const, label: 'Dark', icon: <Moon size={20} /> },
                        { value: 'system' as const, label: 'System', icon: <Monitor size={20} /> },
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

                  <SettingSection label="Font Size">
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

                  <SettingSection label="Density">
                    <p className="mb-2 text-[12px] text-[var(--text-secondary)]">
                      Control the spacing between elements
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
                          {d}
                        </button>
                      ))}
                    </div>
                  </SettingSection>
                </div>
              )}

              {activeTab === 'security' && (
                <div className="flex flex-col gap-6">
                  <SettingSection label="Master Password">
                    <p className="mb-3 text-[12px] text-[var(--text-secondary)]">
                      Change your vault master password
                    </p>
                    <button
                      onClick={() => setShowChangePassword(true)}
                      className="h-8 rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] px-3 text-[13px] font-medium text-[var(--text-primary)] transition-colors hover:bg-[var(--bg-hover)]"
                    >
                      Change Password...
                    </button>
                  </SettingSection>

                  <SettingSection label="Security Audit">
                    <SecurityDashboard />
                  </SettingSection>
                </div>
              )}

              {activeTab === 'backup' && (
                <div className="flex flex-col gap-6">
                  <SettingSection label="Export">
                    <p className="mb-3 text-[12px] text-[var(--text-secondary)]">
                      Export your vault to an encrypted file
                    </p>
                    <button className="h-8 rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] px-3 text-[13px] font-medium text-[var(--text-primary)] transition-colors hover:bg-[var(--bg-hover)]">
                      Export Vault...
                    </button>
                  </SettingSection>

                  <SettingSection label="Import">
                    <p className="mb-3 text-[12px] text-[var(--text-secondary)]">
                      Import entries from a file
                    </p>
                    <p className="mb-3 text-[12px] text-[var(--destructive)]">
                      This will merge with existing entries
                    </p>
                    <button className="h-8 rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] px-3 text-[13px] font-medium text-[var(--text-primary)] transition-colors hover:bg-[var(--bg-hover)]">
                      Import from File...
                    </button>
                  </SettingSection>
                </div>
              )}

              {activeTab === 'trash' && (
                <div className="flex flex-col gap-6">
                  <SettingSection label="Trash Management">
                    <p className="mb-3 text-[12px] text-[var(--text-secondary)]">
                      View items moved to the trash. Trashed items are automatically deleted permanently after 30 days.
                    </p>
                    {trashItems.length > 0 && (
                      <button
                        onClick={handleEmptyTrash}
                        className="mb-4 flex items-center gap-1.5 rounded-[3px] border border-red-500/30 bg-red-500/10 px-3 py-1.5 text-[12px] font-medium text-red-500 transition-colors hover:bg-red-500 hover:text-white"
                      >
                        <Trash size={13} />
                        Empty Trash
                      </button>
                    )}

                    {loadingTrash ? (
                      <p className="text-[12px] text-[var(--text-tertiary)] py-4">Loading trash...</p>
                    ) : trashItems.length === 0 ? (
                      <div className="flex flex-col items-center justify-center gap-2 py-8 rounded-[3px] border border-dashed border-[var(--border-subtle)]">
                        <Trash2 size={20} className="text-[var(--text-tertiary)]" />
                        <p className="text-[12px] text-[var(--text-tertiary)]">Trash is empty</p>
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
                              <button
                                onClick={() => handleRestore(item.id)}
                                className="inline-flex h-7 items-center gap-1 rounded-[3px] px-2 text-[11px] font-medium text-[var(--text-secondary)] transition-colors hover:bg-[var(--bg-active)] hover:text-[var(--text-primary)]"
                                title="Restore to vault"
                              >
                                <RotateCcw size={12} />
                                Restore
                              </button>
                              <button
                                onClick={() => handlePermanentDelete(item.id)}
                                className="inline-flex h-7 items-center gap-1 rounded-[3px] px-2 text-[11px] font-medium text-[var(--destructive)] transition-colors hover:bg-[var(--destructive)]/10"
                                title="Delete permanently"
                              >
                                <Trash2 size={12} />
                                Delete
                              </button>
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



