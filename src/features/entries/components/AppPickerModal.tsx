/**
 * AppPickerModal — SOTA Installed Application Picker & Categorized Filter
 * 
 * Scans installed desktop applications, filters out non-login OS utilities,
 * provides category tabs, and fallbacks to manual OS file browser.
 */

import { useState, useEffect } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { X, Search, FolderOpen, AppWindow, Laptop, Filter, ShieldAlert, Check } from 'lucide-react';
import { useTranslation } from '@/contexts/LanguageContext';
import { isTauri, getBackend, openFileDialog } from '@/lib/backend';
import type { InstalledApp } from '@/types';
import { ActionTooltip } from '@/components/ui/tooltip';

export interface AppPickerModalProps {
  open: boolean;
  onClose: () => void;
  onSelectApp: (appPath: string) => void;
}

export type AppCategoryTab = 'loggable' | 'all' | 'browsers_communication' | 'productivity_dev' | 'gaming' | 'system';

export const COMMON_APPS: InstalledApp[] = [
  // Browsers & Communication
  { name: 'Google Chrome', path: 'googlechrome://', category: 'browsers_communication', is_system: false },
  { name: 'Mozilla Firefox', path: 'firefox://', category: 'browsers_communication', is_system: false },
  { name: 'Discord', path: 'discord://', category: 'browsers_communication', is_system: false },
  { name: 'Slack', path: 'slack://', category: 'browsers_communication', is_system: false },
  { name: 'Telegram', path: 'tg://', category: 'browsers_communication', is_system: false },
  { name: 'WhatsApp', path: 'whatsapp://', category: 'browsers_communication', is_system: false },
  { name: 'Signal', path: 'sgnl://', category: 'browsers_communication', is_system: false },
  { name: 'Microsoft Teams', path: 'msteams://', category: 'browsers_communication', is_system: false },
  { name: 'Thunderbird', path: 'thunderbird://', category: 'browsers_communication', is_system: false },

  // Dev & Productivity
  { name: 'Visual Studio Code', path: 'vscode://', category: 'productivity_dev', is_system: false },
  { name: 'Cursor', path: 'cursor://', category: 'productivity_dev', is_system: false },
  { name: 'Windsurf', path: 'windsurf://', category: 'productivity_dev', is_system: false },
  { name: 'Antigravity', path: 'antigravity://', category: 'productivity_dev', is_system: false },
  { name: 'Obsidian', path: 'obsidian://', category: 'productivity_dev', is_system: false },
  { name: 'Notion', path: 'notion://', category: 'productivity_dev', is_system: false },
  { name: 'Linear', path: 'linear://', category: 'productivity_dev', is_system: false },
  { name: 'Docker Desktop', path: 'docker://', category: 'productivity_dev', is_system: false },
  { name: 'Postman', path: 'postman://', category: 'productivity_dev', is_system: false },
  { name: 'DBeaver', path: 'dbeaver://', category: 'productivity_dev', is_system: false },
  { name: 'GitKraken', path: 'gitkraken://', category: 'productivity_dev', is_system: false },
  { name: 'Figma', path: 'figma://', category: 'productivity_dev', is_system: false },

  // Gaming & Media
  { name: 'Steam', path: 'steam://', category: 'gaming', is_system: false },
  { name: 'Spotify', path: 'spotify://', category: 'gaming', is_system: false },
  { name: 'NVIDIA App', path: 'nvidia://', category: 'gaming', is_system: false },
  { name: 'Epic Games Store', path: 'com.epicgames.launcher://', category: 'gaming', is_system: false },
  { name: 'Battle.net', path: 'battlenet://', category: 'gaming', is_system: false },
  { name: 'Roblox', path: 'roblox://', category: 'gaming', is_system: false },
  { name: 'VLC Media Player', path: 'vlc://', category: 'gaming', is_system: false },
];

export function AppPickerModal({ open, onClose, onSelectApp }: AppPickerModalProps) {
  const { t } = useTranslation();
  const [query, setQuery] = useState('');
  const [activeTab, setActiveTab] = useState<AppCategoryTab>('loggable');
  const [hideSystemUtils, setHideSystemUtils] = useState(true);
  const [installedApps, setInstalledApps] = useState<InstalledApp[]>([]);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (!open) return;
    setQuery('');
    setActiveTab('loggable');

    let cancelled = false;
    async function loadApps() {
      setLoading(true);
      try {
        if (isTauri()) {
          const backend = await getBackend();
          if ('getInstalledApps' in backend) {
            const apps = await (backend as any).getInstalledApps();
            if (!cancelled && apps && apps.length > 0) {
              setInstalledApps(apps);
            }
          }
        }
      } catch {
        // Fallback to common presets if scan is not available
      } finally {
        if (!cancelled) setLoading(false);
      }
    }
    loadApps();
    return () => { cancelled = true; };
  }, [open]);

  const rawApps = installedApps.length > 0 ? installedApps : COMMON_APPS;

  const filteredApps = rawApps.filter(app => {
    // Search query filter
    const matchesQuery = !query.trim() ||
      app.name.toLowerCase().includes(query.toLowerCase()) ||
      app.path.toLowerCase().includes(query.toLowerCase());

    if (!matchesQuery) return false;

    // System utility noise filter toggle
    if (hideSystemUtils && app.is_system && activeTab !== 'system') {
      return false;
    }

    // Category Tab filter
    if (activeTab === 'loggable') {
      return hideSystemUtils ? !app.is_system : true;
    }
    if (activeTab === 'browsers_communication') {
      return app.category === 'browsers_communication';
    }
    if (activeTab === 'productivity_dev') {
      return app.category === 'productivity_dev';
    }
    if (activeTab === 'gaming') {
      return app.category === 'gaming';
    }
    if (activeTab === 'system') {
      return app.is_system || app.category === 'system';
    }

    return true; // 'all' tab
  });

  const handleManualBrowse = async () => {
    try {
      const selected = await openFileDialog({
        multiple: false,
        filters: [{ name: t('app_picker.dialog_filter_apps'), extensions: ['exe', 'app', 'desktop', 'bat', 'cmd', 'lnk', '*'] }],
      });
      const appPath = typeof selected === 'string' ? selected : Array.isArray(selected) ? selected[0] : null;
      if (appPath) {
        onSelectApp(appPath);
        onClose();
      }
    } catch {
      // Fallback
    }
  };

  const getCategoryBadge = (cat?: string, isSys?: boolean) => {
    if (isSys) return <span className="rounded bg-[var(--accent-bg)] px-1.5 py-0.5 text-[10px] font-medium text-[var(--text-secondary)] border border-[var(--border)] whitespace-nowrap shrink-0">{t('app_picker.badge_system')}</span>;
    if (cat === 'browsers_communication') return <span className="rounded bg-[var(--accent-bg)] px-1.5 py-0.5 text-[10px] font-medium text-[var(--text-secondary)] border border-[var(--border)] whitespace-nowrap shrink-0">{t('app_picker.badge_browsers')}</span>;
    if (cat === 'productivity_dev') return <span className="rounded bg-[var(--accent-bg)] px-1.5 py-0.5 text-[10px] font-medium text-[var(--text-secondary)] border border-[var(--border)] whitespace-nowrap shrink-0">{t('app_picker.badge_dev')}</span>;
    if (cat === 'gaming') return <span className="rounded bg-[var(--accent-bg)] px-1.5 py-0.5 text-[10px] font-medium text-[var(--text-secondary)] border border-[var(--border)] whitespace-nowrap shrink-0">{t('app_picker.badge_gaming')}</span>;
    return <span className="rounded bg-[var(--accent-bg)] px-1.5 py-0.5 text-[10px] font-medium text-[var(--text-tertiary)] border border-[var(--border-subtle)] whitespace-nowrap shrink-0">{t('app_picker.badge_app')}</span>;
  };

  return (
    <AnimatePresence>
      {open && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-xs p-4 select-none" onClick={onClose}>
          <motion.div
            initial={{ opacity: 0, scale: 0.97 }}
            animate={{ opacity: 1, scale: 1 }}
            exit={{ opacity: 0, scale: 0.97 }}
            transition={{ duration: 0.15 }}
            className="w-full max-w-xl mx-3 rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] shadow-2xl flex flex-col max-h-[85vh] overflow-hidden"
            onClick={(e) => e.stopPropagation()}
          >
            {/* Header */}
            <div className="flex items-center justify-between border-b border-[var(--border)] px-4 py-3 bg-[var(--bg-surface)]">
              <div className="flex items-center gap-2.5">
                <div className="flex h-7 w-7 items-center justify-center rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] text-[var(--text-primary)]">
                  <Laptop size={14} />
                </div>
                <span className="text-[13px] font-semibold text-[var(--text-primary)]">{t('app_picker.title')}</span>
              </div>
              <button
                type="button"
                onClick={onClose}
                className="rounded-[3px] p-1 text-[var(--text-tertiary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)] transition-colors cursor-pointer"
              >
                <X size={15} />
              </button>
            </div>

            {/* Content Body */}
            <div className="flex flex-col flex-1 min-h-0 p-4">
              {/* Search & Manual Browse Bar */}
              <div className="flex items-center gap-2">
                <div className="relative flex-1">
                  <Search size={14} className="absolute left-3 top-1/2 -translate-y-1/2 text-[var(--text-tertiary)]" />
                  <input
                    type="text"
                    value={query}
                    onChange={(e) => setQuery(e.target.value)}
                    placeholder={t('app_picker.search_placeholder')}
                    className="h-8 w-full rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] pl-9 pr-3 text-[12px] text-[var(--text-primary)] outline-none placeholder:text-[var(--text-tertiary)] focus:border-[var(--border-focus)] transition-colors"
                    autoFocus
                  />
                </div>

                <ActionTooltip content={t('app_picker.filter_noise_tooltip')}>
                  <button
                    type="button"
                    onClick={() => setHideSystemUtils(!hideSystemUtils)}
                    className={`flex h-8 items-center gap-1.5 rounded-[3px] border px-2.5 text-[11.5px] font-medium transition-colors shrink-0 cursor-pointer ${
                      hideSystemUtils
                        ? 'border-[var(--text-primary)] bg-[var(--bg-active)] text-[var(--text-primary)]'
                        : 'border-[var(--border)] bg-[var(--bg-base)] text-[var(--text-tertiary)] hover:text-[var(--text-primary)]'
                    }`}
                  >
                    <Filter size={13} />
                    <span className="hidden sm:inline">{t('app_picker.filter_noise')}</span>
                    {hideSystemUtils && <Check size={12} />}
                  </button>
                </ActionTooltip>

                <ActionTooltip content={t('app_picker.browse_tooltip')}>
                  <button
                    type="button"
                    onClick={handleManualBrowse}
                    className="flex h-8 items-center gap-1.5 rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] px-3 text-[11.5px] font-medium text-[var(--text-secondary)] transition-colors hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)] shrink-0 cursor-pointer"
                  >
                    <FolderOpen size={13} />
                    <span className="hidden sm:inline">{t('app_picker.browse_file_system')}</span>
                  </button>
                </ActionTooltip>
              </div>

              {/* Category Tabs */}
              <div
                onWheel={(e) => {
                  if (e.deltaY) {
                    e.currentTarget.scrollLeft += e.deltaY;
                  }
                }}
                className="mt-3 flex items-center gap-1.5 overflow-x-auto border-b border-[var(--border-subtle)] pb-2.5 text-[11px] scrollbar-none shrink-0"
              >
                {[
                  { id: 'loggable' as const, label: t('app_picker.tab_loggable') },
                  { id: 'browsers_communication' as const, label: t('app_picker.tab_browsers') },
                  { id: 'productivity_dev' as const, label: t('app_picker.tab_dev') },
                  { id: 'gaming' as const, label: t('app_picker.tab_gaming') },
                  { id: 'system' as const, label: t('app_picker.tab_system') },
                  { id: 'all' as const, label: `${t('app_picker.tab_all')} (${rawApps.length})` },
                ].map((tab) => (
                  <button
                    key={tab.id}
                    type="button"
                    onClick={() => setActiveTab(tab.id)}
                    className={`rounded-[3px] px-2.5 py-1 font-medium transition-colors whitespace-nowrap cursor-pointer ${
                      activeTab === tab.id
                        ? 'bg-[var(--text-primary)] text-[var(--bg-base)] shadow-2xs'
                        : 'border border-[var(--border)] bg-[var(--bg-base)] text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]'
                    }`}
                  >
                    {tab.label}
                  </button>
                ))}
              </div>

              {/* Apps List */}
              <div className="mt-3 flex-1 overflow-y-auto pr-1 space-y-1 min-h-[240px]">
                {loading ? (
                  <div className="flex h-40 items-center justify-center text-[12px] text-[var(--text-tertiary)]">
                    {t('app_picker.scanning')}
                  </div>
                ) : filteredApps.length === 0 ? (
                  <div className="flex flex-col items-center justify-center h-48 gap-2 text-[12px] text-[var(--text-tertiary)] text-center">
                    <ShieldAlert size={24} className="text-[var(--text-tertiary)] opacity-60" />
                    <span>{t('app_picker.no_apps_found')}</span>
                    <div className="flex items-center gap-3 mt-1">
                      {hideSystemUtils && (
                        <button
                          type="button"
                          onClick={() => setHideSystemUtils(false)}
                          className="text-[12px] text-[var(--text-primary)] hover:underline font-medium cursor-pointer"
                        >
                          {t('app_picker.show_os_utilities')}
                        </button>
                      )}
                      <button
                        type="button"
                        onClick={handleManualBrowse}
                        className="text-[12px] text-[var(--text-primary)] hover:underline font-medium cursor-pointer"
                      >
                        {t('app_picker.browse_file_system')}
                      </button>
                    </div>
                  </div>
                ) : (
                  filteredApps.map((app, idx) => (
                    <button
                      key={`${app.name}-${idx}`}
                      type="button"
                      onClick={() => {
                        onSelectApp(app.path);
                        onClose();
                      }}
                      className="flex w-full items-center justify-between rounded-[3px] p-2 text-left transition-colors hover:bg-[var(--bg-hover)] group cursor-pointer border border-transparent hover:border-[var(--border-subtle)]"
                    >
                      <div className="flex items-center gap-2.5 min-w-0 flex-1">
                        <div className="flex h-7 w-7 items-center justify-center rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] text-[var(--text-secondary)] group-hover:text-[var(--text-primary)] shrink-0">
                          <AppWindow size={14} />
                        </div>
                        <div className="min-w-0 flex-1">
                          <div className="flex items-center gap-2 min-w-0">
                            <span className="text-[12px] font-medium text-[var(--text-primary)] truncate min-w-0">
                              {app.name}
                            </span>
                            {getCategoryBadge(app.category, app.is_system)}
                          </div>
                          <ActionTooltip content={app.path}>
                            <div className="text-[10.5px] font-mono text-[var(--text-tertiary)] truncate mt-0.5 max-w-full">
                              {app.path}
                            </div>
                          </ActionTooltip>
                        </div>
                      </div>
                      <div className="text-[11px] font-medium text-[var(--text-tertiary)] group-hover:text-[var(--text-secondary)] shrink-0 pl-2">
                        {t('app_picker.select')}
                      </div>
                    </button>
                  ))
                )}
              </div>
            </div>
          </motion.div>
        </div>
      )}
    </AnimatePresence>
  );
}

export default AppPickerModal;
