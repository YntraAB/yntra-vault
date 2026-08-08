import { useState, useEffect, useCallback, useMemo } from 'react';
import { motion } from 'framer-motion';
import { RotateCcw, Keyboard, AlertTriangle, Edit2, X } from 'lucide-react';
import { useAppState } from '@/contexts/AppStateContext';
import { useTranslation } from '@/contexts/LanguageContext';
import { SettingSection } from './SettingSection';
import { ActionTooltip } from '../ui/tooltip';
import {
  DEFAULT_KEYBINDS,
  formatShortcutKeys,
  shortcutsEqual,
  getKeybinds,
} from '@/lib/keybinds';
import type { KeybindShortcut, KeybindsConfig } from '@/types';

type KeybindActionKey = keyof KeybindsConfig;

interface KeybindActionItem {
  id: KeybindActionKey;
  label: string;
  description: string;
}

interface KeybindCategory {
  title: string;
  items: KeybindActionItem[];
}

export function KeybindsTab() {
  const { settings, updateSettings, addToast } = useAppState();
  const { t } = useTranslation();
  const currentKeybinds: KeybindsConfig = getKeybinds(settings.keybinds);

  const categories: KeybindCategory[] = useMemo(
    () => [
      {
        title: t('keybinds.cat_global'),
        items: [
          {
            id: 'search',
            label: t('keybinds.search_label'),
            description: t('keybinds.search_desc'),
          },
          {
            id: 'newEntry',
            label: t('keybinds.newEntry_label'),
            description: t('keybinds.newEntry_desc'),
          },
          {
            id: 'lockVault',
            label: t('keybinds.lockVault_label'),
            description: t('keybinds.lockVault_desc'),
          },
        ],
      },
      {
        title: t('keybinds.cat_clipboard'),
        items: [
          {
            id: 'copyPassword',
            label: t('keybinds.copyPassword_label'),
            description: t('keybinds.copyPassword_desc'),
          },
          {
            id: 'copyUsername',
            label: t('keybinds.copyUsername_label'),
            description: t('keybinds.copyUsername_desc'),
          },
          {
            id: 'copyUrl',
            label: t('keybinds.copyUrl_label'),
            description: t('keybinds.copyUrl_desc'),
          },
          {
            id: 'copyTotp',
            label: t('keybinds.copyTotp_label'),
            description: t('keybinds.copyTotp_desc'),
          },
        ],
      },
      {
        title: t('keybinds.cat_entry'),
        items: [
          {
            id: 'editEntry',
            label: t('keybinds.editEntry_label'),
            description: t('keybinds.editEntry_desc'),
          },
          {
            id: 'deleteEntry',
            label: t('keybinds.deleteEntry_label'),
            description: t('keybinds.deleteEntry_desc'),
          },
          {
            id: 'openUrl',
            label: t('keybinds.openUrl_label'),
            description: t('keybinds.openUrl_desc'),
          },
          {
            id: 'autotype',
            label: t('keybinds.autotype_label'),
            description: t('keybinds.autotype_desc'),
          },
        ],
      },
    ],
    [t]
  );

  const allItems = useMemo(() => categories.flatMap((c) => c.items), [categories]);

  const [recordingAction, setRecordingAction] = useState<KeybindActionKey | null>(null);
  const [recordedKeys, setRecordedKeys] = useState<KeybindShortcut | null>(null);

  // Key capture listener during recording mode
  useEffect(() => {
    if (!recordingAction) return;

    const handleKeyDown = (e: KeyboardEvent) => {
      e.preventDefault();
      e.stopPropagation();

      if (e.key === 'Escape') {
        setRecordingAction(null);
        setRecordedKeys(null);
        return;
      }

      // Ignore single modifier key presses by themselves
      const modifierOnly = ['Control', 'Shift', 'Alt', 'Meta'].includes(e.key);

      const shortcut: KeybindShortcut = {
        ctrlKey: e.ctrlKey,
        altKey: e.altKey,
        shiftKey: e.shiftKey,
        metaKey: e.metaKey,
        key: modifierOnly ? '' : e.key === ' ' ? 'Space' : e.key,
      };

      setRecordedKeys(shortcut);

      // Save as soon as a non-modifier key is pressed
      if (!modifierOnly && shortcut.key) {
        // Check for conflicts across all actions
        const conflictAction = allItems.find(
          (act) => act.id !== recordingAction && shortcutsEqual(currentKeybinds[act.id], shortcut)
        );

        const newConfig: KeybindsConfig = {
          ...currentKeybinds,
          [recordingAction]: shortcut,
        };

        updateSettings({ keybinds: newConfig });
        setRecordingAction(null);
        setRecordedKeys(null);

        if (conflictAction) {
          addToast({
            message: `Shortcut saved (Conflicts with "${conflictAction.label}")`,
            type: 'info',
          });
        } else {
          addToast({ message: 'Shortcut updated successfully', type: 'info' });
        }
      }
    };

    window.addEventListener('keydown', handleKeyDown, true);
    return () => window.removeEventListener('keydown', handleKeyDown, true);
  }, [recordingAction, currentKeybinds, updateSettings, addToast, allItems]);

  const handleResetDefaults = useCallback(() => {
    updateSettings({ keybinds: DEFAULT_KEYBINDS });
    addToast({ message: 'Reset shortcuts to default', type: 'info' });
  }, [updateSettings, addToast]);

  return (
    <div className="flex flex-col gap-6">
      {/* Header Info */}
      <div className="flex items-center justify-between rounded-[4px] border border-[var(--border-subtle)] bg-[var(--bg-elevated)] p-3.5">
        <div className="flex items-center gap-3">
          <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-[3px] bg-[var(--bg-hover)] text-[var(--text-primary)]">
            <Keyboard size={18} />
          </div>
          <div>
            <div className="text-[13px] font-semibold text-[var(--text-primary)]">
              {t('keybinds.title')}
            </div>
            <div className="text-[12px] text-[var(--text-secondary)]">
              {t('keybinds.desc')}
            </div>
          </div>
        </div>

        <ActionTooltip content={t('keybinds.reset_tooltip')}>
          <button
            type="button"
            onClick={handleResetDefaults}
            className="inline-flex items-center gap-1.5 rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] px-2.5 py-1 text-[12px] font-medium text-[var(--text-secondary)] transition-colors hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)] active:scale-95"
          >
            <RotateCcw size={13} />
            {t('keybinds.reset_defaults')}
          </button>
        </ActionTooltip>
      </div>

      {/* Categorized Shortcuts */}
      {categories.map((cat) => (
        <SettingSection key={cat.title} label={cat.title}>
          <div className="flex flex-col gap-2">
            {cat.items.map((action) => {
              const currentShortcut = currentKeybinds[action.id] || DEFAULT_KEYBINDS[action.id];
              const defaultShortcut = DEFAULT_KEYBINDS[action.id];
              const isRecording = recordingAction === action.id;
              const isModified = !shortcutsEqual(currentShortcut, defaultShortcut);

              // Check if this action conflicts with another action
              const conflictWith = allItems.find(
                (other) => other.id !== action.id && shortcutsEqual(currentKeybinds[other.id], currentShortcut)
              );

              const displayKeys = isRecording && recordedKeys?.key
                ? formatShortcutKeys(recordedKeys)
                : formatShortcutKeys(currentShortcut);

              return (
                <div
                  key={action.id}
                  className={`flex items-center justify-between rounded-[4px] border p-3 transition-all ${
                    isRecording
                      ? 'border-[var(--border-focus)] bg-[var(--bg-elevated)] shadow-xs'
                      : 'border-[var(--border-subtle)] bg-[var(--bg-elevated)]/50 hover:bg-[var(--bg-elevated)]'
                  }`}
                >
                  <div className="min-w-0 pr-4">
                    <div className="flex items-center gap-2">
                      <span className="text-[13px] font-medium text-[var(--text-primary)]">
                        {action.label}
                      </span>
                      {isModified && (
                        <span className="rounded-[2px] bg-[var(--accent-hover)]/10 px-1.5 py-0.2 text-[10px] font-medium text-[var(--accent-hover)]">
                          {t('keybinds.custom_badge')}
                        </span>
                      )}
                      {conflictWith && !isRecording && (
                        <span className="inline-flex items-center gap-1 text-[11px] font-medium text-amber-500">
                          <AlertTriangle size={12} />
                          {t('keybinds.conflicts_with')} {conflictWith.label}
                        </span>
                      )}
                    </div>
                    <div className="text-[12px] text-[var(--text-secondary)] truncate">
                      {action.description}
                    </div>
                  </div>

                  {/* KBD Display / Recording Pill */}
                  <div className="flex items-center gap-2 shrink-0">
                    {isRecording ? (
                      <motion.div
                        initial={{ scale: 0.96, opacity: 0 }}
                        animate={{ scale: 1, opacity: 1 }}
                        className="flex items-center gap-2"
                      >
                        <div className="flex h-8 items-center gap-1.5 rounded-[4px] border border-[var(--border-focus)] bg-[var(--bg-elevated)] px-2.5 text-[12px] font-medium text-[var(--text-primary)] shadow-xs">
                          <span className="relative flex h-2 w-2">
                            <span className="absolute inline-flex h-full w-full animate-ping rounded-full bg-[var(--accent-hover)] opacity-75" />
                            <span className="relative inline-flex h-2 w-2 rounded-full bg-[var(--accent-hover)]" />
                          </span>
                          {recordedKeys && (recordedKeys.ctrlKey || recordedKeys.altKey || recordedKeys.shiftKey || recordedKeys.metaKey) ? (
                            <div className="flex items-center gap-1">
                              {formatShortcutKeys(recordedKeys).map((k, i, arr) => (
                                <span key={i} className="flex items-center gap-1">
                                  <kbd className="inline-block min-w-[18px] text-center rounded-[2px] border border-[var(--border)] bg-[var(--bg-base)] px-1 py-0.2 font-mono text-[11px] font-semibold text-[var(--text-primary)] shadow-xs">
                                    {k}
                                  </kbd>
                                  {i < arr.length - 1 && <span className="text-[10px] text-[var(--text-tertiary)]">+</span>}
                                </span>
                              ))}
                              <span className="text-[11px] font-mono text-[var(--text-tertiary)] animate-pulse">...</span>
                            </div>
                          ) : (
                            <span>{t('keybinds.press_keys')}</span>
                          )}
                        </div>
                        <ActionTooltip content={t('keybinds.cancel')}>
                          <button
                            type="button"
                            onClick={() => {
                              setRecordingAction(null);
                              setRecordedKeys(null);
                            }}
                            className="flex h-8 w-8 items-center justify-center rounded-[4px] border border-[var(--border)] bg-[var(--bg-base)] text-[11px] font-medium text-[var(--text-tertiary)] transition-colors hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)] active:scale-95"
                          >
                            <X size={14} />
                          </button>
                        </ActionTooltip>
                      </motion.div>
                    ) : (
                      <button
                        type="button"
                        onClick={() => setRecordingAction(action.id)}
                        className="group flex h-8 items-center gap-1 rounded-[4px] border border-[var(--border)] bg-[var(--bg-base)] px-2.5 transition-all hover:border-[var(--border-focus)] hover:bg-[var(--bg-hover)]"
                      >
                        <div className="flex items-center gap-1">
                          {displayKeys.map((k, i) => (
                            <span key={i} className="flex items-center gap-1">
                              <kbd className="inline-block min-w-[20px] text-center rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] px-1.5 py-0.5 font-mono text-[11px] font-semibold text-[var(--text-primary)] shadow-xs">
                                {k}
                              </kbd>
                              {i < displayKeys.length - 1 && (
                                <span className="text-[10px] text-[var(--text-tertiary)]">+</span>
                              )}
                            </span>
                          ))}
                        </div>

                        <Edit2
                          size={12}
                          className="ml-1 text-[var(--text-tertiary)] opacity-0 transition-opacity group-hover:opacity-100"
                        />
                      </button>
                    )}
                  </div>
                </div>
              );
            })}
          </div>
        </SettingSection>
      ))}
    </div>
  );
}
