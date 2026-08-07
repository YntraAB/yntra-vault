import { useState, useEffect, useCallback, Fragment } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import {
  Pencil,
  Trash2,
  User,
  Key,
  Link,
  Mail,
  Globe,
  FileText,
  ShieldCheck,
  Star,
  Pin,
  Eye,
  EyeOff,
  ExternalLink,
  X,
} from 'lucide-react';
import { useAppState } from '@/contexts/AppStateContext';
import { useTranslation } from '@/contexts/LanguageContext';
import CopyButton from './CopyButton';
import AutotypeButton from './AutotypeButton';
import PasswordInput from './PasswordInput';
import { PasswordStrength } from './PasswordStrength';
import { BreachIndicator } from './BreachIndicator';
import DeleteEntryModal from './DeleteEntryModal';
import type { BreachStatus } from '@/lib/backend';
import { useTotp, useBackend } from '@/lib/useBackend';
import EntryModal from './EntryModal';
import Favicon from './Favicon';
import { formatDate, getFieldLayout, openExternalUrl } from '@/lib/utils';
import type { Tag } from '@/types';
import { Skeleton } from './ui/skeleton';
import { ActionTooltip } from './ui/tooltip';
import { matchesShortcut, getKeybinds } from '@/lib/keybinds';

export default function PasswordDetail() {
  const { t } = useTranslation();
  const {
    selectedEntry,
    setIsEditing,
    isEditing,
    deleteEntry,
    updateEntry,
    tags,
    togglePin,
    toggleFavorite,
    isLoadingDetail,
    addToast,
    settings,
    settingsOpen,
    isEntryModalOpen,
    refreshEntries,
    selectEntryById,
    setFilterCategory,
  } = useAppState();
  const { backend } = useBackend();
  const [editData, setEditData] = useState(selectedEntry);
  const [showDelConfirm, setShowDelConfirm] = useState(false);
  const [showEditModal, setShowEditModal] = useState(false);
  const [showRecovery, setShowRecovery] = useState(false);
  const [showTemporaryStats, setShowTemporaryStats] = useState(false);
  const [showPassword, setShowPassword] = useState(false);
  const [showCustomPasswords, setShowCustomPasswords] = useState<Record<string, boolean>>({});
  const [showWarningModal, setShowWarningModal] = useState(false);
  const [warningTimer, setWarningTimer] = useState(0);
  const [dontShowAgain, setDontShowAgain] = useState(false);

  const runSmartLoginAction = useCallback(async () => {
    if (!backend || !selectedEntry) return;
    addToast({
      message: 'Smart Login active! Focus a username or password input in your browser.',
      type: 'info'
    });
    try {
      await backend.runSmartAutotype(
        selectedEntry.username,
        selectedEntry.password,
        selectedEntry.totpSecret || '',
        selectedEntry.url || '',
        settings.autotypeLaunchBrowser !== false,
        settings.autotypeCharDelayMs || 15,
        settings.autotypeFieldDelayMs || 300
      );
    } catch (err) {
      addToast({ message: `Smart Login failed: ${err}`, type: 'error' });
    }
  }, [backend, selectedEntry, settings.autotypeLaunchBrowser, settings.autotypeCharDelayMs, settings.autotypeFieldDelayMs, addToast]);

  // Keyboard shortcuts to copy entry details (Password, Username, URL, TOTP) using single configured keybinds
  useEffect(() => {
    if (!selectedEntry || isEditing) return;

    const handleCopyShortcuts = async (e: KeyboardEvent) => {
      if (settingsOpen || isEntryModalOpen || showDelConfirm || showWarningModal || showEditModal) {
        return;
      }

      const hasOpenDialog = Boolean(
        document.querySelector('[role="dialog"], [aria-modal="true"], dialog[open], .fixed.inset-0')
      );
      if (hasOpenDialog) return;

      const kb = getKeybinds(settings.keybinds);
      const selection = window.getSelection()?.toString() || '';
      const hasSelection = selection.length > 0;
      const isInputFocused =
        document.activeElement instanceof HTMLInputElement ||
        document.activeElement instanceof HTMLTextAreaElement ||
        document.activeElement?.getAttribute('contenteditable') === 'true';

      // 1. Copy Password
      if (matchesShortcut(e, kb.copyPassword)) {
        if (!hasSelection && !isInputFocused && selectedEntry.password) {
          e.preventDefault();
          e.stopPropagation();
          navigator.clipboard.writeText(selectedEntry.password).catch(() => {});
          addToast({ message: 'Password copied to clipboard', type: 'info' });
        }
        return;
      }

      // 2. Copy Username
      if (matchesShortcut(e, kb.copyUsername)) {
        if (selectedEntry.username) {
          e.preventDefault();
          e.stopPropagation();
          navigator.clipboard.writeText(selectedEntry.username).catch(() => {});
          addToast({ message: 'Username copied to clipboard', type: 'info' });
        }
        return;
      }

      // 3. Copy Website URL
      if (matchesShortcut(e, kb.copyUrl)) {
        if (selectedEntry.url) {
          e.preventDefault();
          e.stopPropagation();
          navigator.clipboard.writeText(selectedEntry.url).catch(() => {});
          addToast({ message: 'Website URL copied to clipboard', type: 'info' });
        }
        return;
      }

      // 4. Copy TOTP / 2FA Code
      if (matchesShortcut(e, kb.copyTotp)) {
        if (selectedEntry.totpSecret) {
          e.preventDefault();
          e.stopPropagation();
          if (backend) {
            try {
              const totpRes = await backend.generateTotp(selectedEntry.totpSecret);
              if (totpRes && totpRes.code) {
                await navigator.clipboard.writeText(totpRes.code);
                addToast({ message: `TOTP code (${totpRes.code}) copied to clipboard`, type: 'info' });
              }
            } catch {
              addToast({ message: 'Failed to generate TOTP code', type: 'error' });
            }
          }
        }
        return;
      }

      // 5. Edit Entry
      if (matchesShortcut(e, kb.editEntry)) {
        e.preventDefault();
        e.stopPropagation();
        setShowEditModal(true);
        return;
      }

      // 6. Delete Entry
      if (matchesShortcut(e, kb.deleteEntry)) {
        e.preventDefault();
        e.stopPropagation();
        setShowDelConfirm(true);
        return;
      }

      // 7. Open Website URL in Browser
      if (matchesShortcut(e, kb.openUrl)) {
        if (selectedEntry.url) {
          e.preventDefault();
          e.stopPropagation();
          openExternalUrl(selectedEntry.url);
        }
        return;
      }

      // 8. Smart Login / Autotype
      if (matchesShortcut(e, kb.autotype)) {
        e.preventDefault();
        e.stopPropagation();
        runSmartLoginAction();
        return;
      }
    };

    window.addEventListener('keydown', handleCopyShortcuts, true);
    return () => window.removeEventListener('keydown', handleCopyShortcuts, true);
  }, [
    selectedEntry,
    isEditing,
    settings.keybinds,
    settingsOpen,
    isEntryModalOpen,
    showDelConfirm,
    showWarningModal,
    showEditModal,
    addToast,
    backend,
    runSmartLoginAction,
  ]);

  useEffect(() => {
    let intervalId: any = null;
    if (showWarningModal && warningTimer > 0) {
      intervalId = setInterval(() => {
        setWarningTimer((prev) => prev - 1);
      }, 1000);
    }
    return () => {
      if (intervalId) clearInterval(intervalId);
    };
  }, [showWarningModal, warningTimer]);

  useEffect(() => {
    if (!showWarningModal && !showDelConfirm) return;
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        setShowWarningModal(false);
        setShowDelConfirm(false);
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [showWarningModal, showDelConfirm]);

  useEffect(() => {
    if (selectedEntry) {
      setEditData({ ...selectedEntry });
      setShowRecovery(false);
      setShowPassword(false);

      // Check if entry was newly created/updated (within 5 seconds)
      const ageMs = Date.now() - new Date(selectedEntry.updatedAt).getTime();
      if (ageMs < 5000) {
        setShowTemporaryStats(true);
        const timer = setTimeout(() => {
          setShowTemporaryStats(false);
        }, 3000);
        return () => clearTimeout(timer);
      } else {
        setShowTemporaryStats(false);
      }
    }
  }, [selectedEntry, isEditing]);

  const handleSave = useCallback(() => {
    if (editData) {
      updateEntry(editData);
      setIsEditing(false);
    }
  }, [editData, setIsEditing, updateEntry]);



  const handleSmartLoginClick = useCallback(() => {
    const skipWarning = localStorage.getItem('yntra-vault-skip-smart-login-warning') === 'true';
    if (skipWarning) {
      runSmartLoginAction();
    } else {
      setDontShowAgain(false);
      setWarningTimer(3);
      setShowWarningModal(true);
    }
  }, [runSmartLoginAction]);

  const data = isEditing && editData ? editData : selectedEntry;
  const entryTags = data
    ? (data.tags.map((t) => tags.find((tag) => tag.name === t)).filter(Boolean) as Tag[])
    : [];

  const activeStandard: string[] = [];
  if (data) {
    if (data.username) activeStandard.push('username');
    if (data.password) activeStandard.push('password');
    if (data.url) activeStandard.push('url');
    if (data.email) activeStandard.push('email');
    if (data.notes) activeStandard.push('notes');
    if (data.totpSecret && data.totpSecret !== 'has-totp') activeStandard.push('totpSecret');
  }

  const displayCustomFields = data ? data.customFields.filter(cf => cf.name !== '_field_order') : [];
  const layoutOrder = data ? getFieldLayout(data.customFields, activeStandard) : [];

  let fieldsContainerClass = 'flex flex-col gap-[2px] p-4';
  let fieldItemPaddingClass = 'px-3 py-2.5';
  if (settings.density === 'compact') {
    fieldsContainerClass = 'flex flex-col gap-[1px] p-2.5';
    fieldItemPaddingClass = 'px-2.5 py-1.5';
  } else if (settings.density === 'comfortable') {
    fieldsContainerClass = 'flex flex-col gap-2 p-6';
    fieldItemPaddingClass = 'px-3.5 py-3.5';
  }

  return (
    <div className="flex h-full flex-col overflow-y-auto">
      <AnimatePresence mode="wait">
        {isLoadingDetail ? (
          <motion.div
            key="loading-skeleton"
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            transition={{ duration: 0.1, ease: 'easeInOut' }}
            className="flex flex-col"
          >
            {/* Header */}
            <div className="flex items-start justify-between border-b border-[var(--border-subtle)] p-4">
              <div className="flex items-start gap-3 w-full">
                <Skeleton className="h-9 w-9 rounded-[3px] shrink-0" />
                <div className="flex flex-col gap-2 flex-1 min-w-0">
                  <Skeleton className="h-5 w-40 rounded" />
                  <Skeleton className="h-4 w-60 rounded" />
                  <div className="mt-1 flex gap-1">
                    <Skeleton className="h-5 w-14 rounded-[2px]" />
                    <Skeleton className="h-5 w-16 rounded-[2px]" />
                  </div>
                </div>
              </div>
            </div>

            {/* Fields */}
            <div className="flex flex-col gap-[2px] p-4">
              {[...Array(5)].map((_, i) => (
                <div key={i} className="flex items-center gap-3 rounded-[3px] bg-[var(--bg-elevated)] px-3 py-2.5">
                  <Skeleton className="h-6 w-6 shrink-0" />
                  <div className="flex-1 min-w-0">
                    <Skeleton className="h-4 w-32 rounded" />
                  </div>
                </div>
              ))}
            </div>
          </motion.div>
        ) : !selectedEntry ? (
          <motion.div
            key="no-selection"
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            transition={{ duration: 0.1, ease: 'easeInOut' }}
            className="flex h-[80vh] flex-col items-center justify-center"
          >
            <div className="text-center">
              <p className="text-[16px] font-semibold text-[var(--text-tertiary)]">{t('detail.select_entry')}</p>
              <p className="mt-1 text-[13px] text-[var(--text-tertiary)]">
                {t('detail.select_entry_desc')}
              </p>
            </div>
          </motion.div>
        ) : !data ? null : (
          <motion.div
            key={data.id}
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            transition={{ duration: 0.1, ease: 'easeInOut' }}
            className="flex flex-col"
          >
            {/* Header */}
            <div className="flex items-start justify-between border-b border-[var(--border-subtle)] p-4">
              <div className="flex items-start gap-3">
                {/* Favicon */}
                <Favicon
                  url={data.url}
                  title={data.title}
                  color={entryTags[0]?.color}
                  sizeClass="h-9 w-9"
                  textClass="text-[12px]"
                />

                <div className="min-w-0">
                  <h1 className="text-[20px] font-semibold leading-tight tracking-tight text-[var(--text-primary)]">
                    {isEditing && editData ? (
                      <input
                        type="text"
                        value={editData.title}
                        onChange={(e) => setEditData({ ...editData, title: e.target.value })}
                        className="w-full rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] px-2 py-1 text-[20px] font-semibold outline-none focus:border-[var(--border-focus)]"
                      />
                    ) : (
                      data.title
                    )}
                  </h1>
                  {data.url && !isEditing && (
                    <a
                      href={/^https?:\/\//i.test(data.url) ? data.url : `https://${data.url}`}
                      target="_blank"
                      rel="noopener noreferrer"
                      onClick={(e) => {
                        e.preventDefault();
                        e.stopPropagation();
                        openExternalUrl(data.url);
                      }}
                      className="mt-0.5 block truncate text-[12px] text-[var(--text-secondary)] transition-colors hover:text-[var(--text-primary)]"
                    >
                      {data.url}
                    </a>
                  )}
                  {isEditing && editData && (
                    <input
                      type="text"
                      value={editData.url}
                      onChange={(e) => setEditData({ ...editData, url: e.target.value })}
                      className="mt-1 w-full rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] px-2 py-0.5 text-[12px] outline-none focus:border-[var(--border-focus)]"
                    />
                  )}
                  {/* Tags */}
                  <div className="mt-2 flex flex-wrap gap-1">
                    {entryTags.map((tag) => (
                      <ActionTooltip key={tag.id} content={t('detail.filter_by', { tag: tag.name })}>
                        <button
                          type="button"
                          onClick={() => setFilterCategory(tag.name)}
                          className="inline-flex items-center gap-1 rounded-[2px] px-1.5 py-0.5 text-[11px] cursor-pointer transition-opacity hover:opacity-80 focus:outline-none"
                          style={{
                            backgroundColor: `${tag.color}14`,
                            color: tag.color,
                            border: `1px solid ${tag.color}33`,
                          }}
                        >
                          <span className="h-1.5 w-1.5 rounded-full" style={{ backgroundColor: tag.color }} />
                          {tag.name}
                        </button>
                      </ActionTooltip>
                    ))}
                  </div>
                </div>
              </div>

              {/* Actions */}
              <div className="flex items-center gap-1">
                {isEditing ? (
                  <>
                    <button
                      onClick={handleSave}
                      className="h-8 rounded-[3px] bg-[var(--text-primary)] px-3 text-[13px] font-medium text-[var(--bg-base)] transition-colors hover:bg-[var(--accent-hover)]"
                    >
                      {t('common.save')}
                    </button>
                    <button
                      onClick={() => setIsEditing(false)}
                      className="h-8 rounded-[3px] px-3 text-[13px] font-medium text-[var(--text-secondary)] transition-colors hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"
                    >
                      {t('common.cancel')}
                    </button>
                  </>
                ) : (
                  <>
                    {/* Pin toggle */}
                    <ActionTooltip content={data.pinned ? t('menu.unpin') : t('menu.pin')}>
                      <button
                        onClick={() => togglePin(data.id)}
                        className={`inline-flex h-8 w-8 items-center justify-center rounded-[3px] transition-colors ${data.pinned
                          ? 'text-yellow-500 hover:bg-yellow-500/10'
                          : 'text-[var(--text-tertiary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]'
                          }`}
                      >
                        <Pin size={15} className={data.pinned ? 'fill-current' : ''} />
                      </button>
                    </ActionTooltip>

                    {/* Favorite toggle */}
                    <ActionTooltip content={data.favorite ? t('detail.fav_remove') : t('detail.fav_add')}>
                      <button
                        onClick={() => toggleFavorite(data.id)}
                        className={`inline-flex h-8 w-8 items-center justify-center rounded-[3px] transition-colors ${data.favorite
                          ? 'text-orange-500 hover:bg-orange-500/10'
                          : 'text-[var(--text-tertiary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]'
                          }`}
                      >
                        <Star size={15} className={data.favorite ? 'fill-current' : ''} />
                      </button>
                    </ActionTooltip>

                    <div className="w-[1px] h-4 bg-[var(--border-subtle)] mx-1" />

                    <ActionTooltip content={t('detail.run_smart_login')}>
                      <button
                        onClick={handleSmartLoginClick}
                        className="inline-flex h-8 items-center rounded-[3px] px-2.5 text-[13px] font-medium text-[var(--text-secondary)] transition-colors hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"
                      >
                        <span>{t('detail.smart_login')}</span>
                      </button>
                    </ActionTooltip>

                    <ActionTooltip content={t('common.edit')}>
                      <button
                        onClick={() => setShowEditModal(true)}
                        className="inline-flex h-8 items-center gap-1.5 rounded-[3px] px-2.5 text-[13px] font-medium text-[var(--text-secondary)] transition-colors hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"
                      >
                        <Pencil size={14} />
                        {t('common.edit')}
                      </button>
                    </ActionTooltip>
                    <ActionTooltip content={t('common.delete')}>
                      <button
                        onClick={() => setShowDelConfirm(true)}
                        className="inline-flex h-8 items-center gap-1.5 rounded-[3px] px-2.5 text-[13px] font-medium text-[var(--destructive)] transition-colors hover:bg-[var(--destructive)]/8"
                      >
                        <Trash2 size={14} />
                        {t('common.delete')}
                      </button>
                    </ActionTooltip>
                  </>
                )}
              </div>
            </div>

            {/* Fields */}
            <div className={fieldsContainerClass}>
              {layoutOrder.map((id, i) => {
                const isStandard = ['username', 'password', 'url', 'email', 'notes', 'totpSecret'].includes(id);

                if (isStandard) {
                  if (id === 'username') {
                    return (
                      <motion.div
                        key="username"
                        initial={{ opacity: 0, y: 2 }}
                        animate={{ opacity: 1, y: 0 }}
                        transition={{ duration: 0.1, delay: i * 0.02 }}
                        className={`flex items-center gap-3 rounded-[3px] bg-[var(--bg-elevated)] ${fieldItemPaddingClass} transition-colors hover:bg-[var(--bg-hover)]`}
                      >
                        <span className="shrink-0 text-[var(--text-secondary)]">
                          <User size={15} />
                        </span>
                        <div className="min-w-0 flex-1">
                          <div className="text-[11px] font-medium uppercase tracking-wide text-[var(--text-tertiary)]">
                            {t('detail.username')}
                          </div>
                          {isEditing && editData ? (
                            <input
                              type="text"
                              value={editData.username}
                              onChange={(e) => setEditData({ ...editData, username: e.target.value })}
                              className="w-full rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] px-2 py-0.5 text-[13px] text-[var(--text-primary)] outline-none focus:border-[var(--border-focus)]"
                            />
                          ) : (
                            <div className="truncate text-[13px] text-[var(--text-primary)]">
                              {data.username}
                            </div>
                          )}
                        </div>
                        {!isEditing && (
                          <div className="flex items-center gap-1">
                            <AutotypeButton value={data.username} />
                            <CopyButton value={data.username} />
                          </div>
                        )}
                      </motion.div>
                    );
                  }

                  if (id === 'password') {
                    return (
                      <div key="password-group" className="flex flex-col gap-2">
                        <motion.div
                          initial={{ opacity: 0, y: 2 }}
                          animate={{ opacity: 1, y: 0 }}
                          transition={{ duration: 0.1, delay: i * 0.02 }}
                          className={`flex items-center gap-3 rounded-[3px] bg-[var(--bg-elevated)] ${fieldItemPaddingClass} transition-colors hover:bg-[var(--bg-hover)]`}
                        >
                          <span className="shrink-0 text-[var(--text-secondary)]">
                            <Key size={15} />
                          </span>
                          <div className="min-w-0 flex-1">
                            <div className="text-[11px] font-medium uppercase tracking-wide text-[var(--text-tertiary)]">
                              {t('detail.password')}
                            </div>
                            {isEditing && editData ? (
                              <PasswordInput
                                value={editData.password}
                                onChange={(v) => setEditData({ ...editData, password: v })}
                              />
                            ) : (
                              <span className="font-mono text-[13px] tracking-wider text-[var(--text-primary)]">
                                {showPassword ? data.password : '••••••••'}
                              </span>
                            )}
                          </div>
                          {!isEditing && (
                            <div className="flex items-center gap-1 shrink-0">
                              <ActionTooltip content={showPassword ? t('login.hide_password') : t('login.show_password')}>
                                <button
                                  type="button"
                                  onClick={() => setShowPassword(!showPassword)}
                                  className="rounded-[3px] p-1 text-[var(--text-tertiary)] transition-colors hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"
                                >
                                  {showPassword ? <EyeOff size={14} /> : <Eye size={14} />}
                                </button>
                              </ActionTooltip>
                              <AutotypeButton value={data.password} />
                              <CopyButton value={data.password} />
                            </div>
                          )}
                        </motion.div>
                        {data.password && !isEditing && (
                          <PasswordSafetySection
                            password={data.password}
                            status={data.breachStatus}
                            showTemporaryStats={showTemporaryStats}
                          />
                        )}
                      </div>
                    );
                  }

                  if (id === 'url') {
                    return (
                      <motion.div
                        key="url"
                        initial={{ opacity: 0, y: 2 }}
                        animate={{ opacity: 1, y: 0 }}
                        transition={{ duration: 0.1, delay: i * 0.02 }}
                        className={`flex items-center gap-3 rounded-[3px] bg-[var(--bg-elevated)] ${fieldItemPaddingClass} transition-colors hover:bg-[var(--bg-hover)]`}
                      >
                        <span className="shrink-0 text-[var(--text-secondary)]">
                          <Link size={15} />
                        </span>
                        <div className="min-w-0 flex-1">
                          <div className="text-[11px] font-medium uppercase tracking-wide text-[var(--text-tertiary)]">
                            {t('detail.url')}
                          </div>
                          {isEditing && editData ? (
                            <input
                              type="text"
                              value={editData.url}
                              onChange={(e) => setEditData({ ...editData, url: e.target.value })}
                              className="w-full rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] px-2 py-0.5 text-[13px] text-[var(--text-primary)] outline-none focus:border-[var(--border-focus)]"
                            />
                          ) : (
                            <div className="truncate text-[13px] text-[var(--text-primary)]">
                              {data.url}
                            </div>
                          )}
                        </div>
                        {!isEditing && (
                          <div className="flex items-center gap-1">
                            {data.url && (
                              <ActionTooltip content={t('detail.open_website')}>
                                <button
                                  type="button"
                                  onClick={(e) => {
                                    e.preventDefault();
                                    e.stopPropagation();
                                    openExternalUrl(data.url);
                                  }}
                                  className="inline-flex items-center justify-center rounded-[3px] p-1 text-[var(--text-tertiary)] transition-all duration-100 hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)] active:scale-95"
                                >
                                  <ExternalLink size={14} />
                                </button>
                              </ActionTooltip>
                            )}
                            <CopyButton value={data.url} />
                          </div>
                        )}
                      </motion.div>
                    );
                  }

                  if (id === 'email') {
                    return (
                      <motion.div
                        key="email"
                        initial={{ opacity: 0, y: 2 }}
                        animate={{ opacity: 1, y: 0 }}
                        transition={{ duration: 0.1, delay: i * 0.02 }}
                        className={`flex items-center gap-3 rounded-[3px] bg-[var(--bg-elevated)] ${fieldItemPaddingClass} transition-colors hover:bg-[var(--bg-hover)]`}
                      >
                        <span className="shrink-0 text-[var(--text-secondary)]">
                          <Mail size={15} />
                        </span>
                        <div className="min-w-0 flex-1">
                          <div className="text-[11px] font-medium uppercase tracking-wide text-[var(--text-tertiary)]">
                            {t('detail.email')}
                          </div>
                          {isEditing && editData ? (
                            <input
                              type="text"
                              value={editData.email}
                              onChange={(e) => setEditData({ ...editData, email: e.target.value })}
                              className="w-full rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] px-2 py-0.5 text-[13px] text-[var(--text-primary)] outline-none focus:border-[var(--border-focus)]"
                            />
                          ) : (
                            <div className="truncate text-[13px] text-[var(--text-primary)]">
                              {data.email}
                            </div>
                          )}
                        </div>
                        {!isEditing && (
                          <div className="flex items-center gap-1">
                            <AutotypeButton value={data.email} />
                            <CopyButton value={data.email} />
                          </div>
                        )}
                      </motion.div>
                    );
                  }

                  if (id === 'notes') {
                    return (
                      <motion.div
                        key="notes"
                        initial={{ opacity: 0, y: 2 }}
                        animate={{ opacity: 1, y: 0 }}
                        transition={{ duration: 0.1, delay: i * 0.02 }}
                        className={`flex items-start gap-3 rounded-[3px] bg-[var(--bg-elevated)] ${fieldItemPaddingClass} transition-colors hover:bg-[var(--bg-hover)]`}
                      >
                        <span className="mt-0.5 shrink-0 text-[var(--text-secondary)]">
                          <FileText size={15} />
                        </span>
                        <div className="min-w-0 flex-1">
                          <div className="text-[11px] font-medium uppercase tracking-wide text-[var(--text-tertiary)]">
                            {t('detail.notes')}
                          </div>
                          {isEditing && editData ? (
                            <textarea
                              value={editData.notes}
                              onChange={(e) => setEditData({ ...editData, notes: e.target.value })}
                              rows={3}
                              className="mt-1 w-full resize-none rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] px-2 py-1 text-[13px] text-[var(--text-primary)] outline-none focus:border-[var(--border-focus)]"
                            />
                          ) : (
                            <p className="whitespace-pre-wrap text-[13px] leading-relaxed text-[var(--text-primary)]">
                              {data.notes}
                            </p>
                          )}
                        </div>
                      </motion.div>
                    );
                  }

                  if (id === 'totpSecret') {
                    return (
                      <Fragment key="totpSecret">
                        <TOTPField
                          secret={data.totpSecret || ''}
                          index={i}
                        />

                        {data.recoveryCodes && (
                          <div className="mt-3 border-t border-[var(--border-subtle)] pt-3">
                            <div className="flex items-center justify-between mb-2">
                              <div className="flex items-center gap-1.5">
                                <span className="text-[11px] font-semibold uppercase tracking-wide text-[var(--text-tertiary)]">
                                  {t('detail.recovery_codes')}
                                </span>
                                <span className="text-[10px] text-[var(--text-tertiary)]/70">
                                  ({data.recoveryCodes.split(/[\s,;\n]+/).filter(Boolean).length} keys)
                                </span>
                              </div>
                              <button
                                onClick={() => setShowRecovery(!showRecovery)}
                                className="text-[11px] font-medium text-[var(--text-secondary)] hover:text-[var(--text-primary)] transition-colors"
                              >
                                {showRecovery ? t('detail.hide_codes') : t('detail.show_codes')}
                              </button>
                            </div>

                            {showRecovery && (
                              <motion.div
                                initial={{ opacity: 0, y: -4 }}
                                animate={{ opacity: 1, y: 0 }}
                                transition={{ duration: 0.1 }}
                                className="space-y-2"
                              >
                                <div className="grid grid-cols-2 gap-1.5 font-mono text-[12px] max-h-36 overflow-y-auto pr-1">
                                  {data.recoveryCodes.split(/[\s,;\n]+/).filter(Boolean).map((code, idx) => (
                                    <RecoveryCodeItem
                                      key={idx}
                                      code={code}
                                      index={idx}
                                      onCopy={() => addToast({ message: `Recovery code ${idx + 1} copied`, type: 'success' })}
                                    />
                                  ))}
                                </div>
                                <div className="flex justify-end">
                                  <button
                                    onClick={() => {
                                      navigator.clipboard.writeText(data.recoveryCodes || '');
                                      addToast({ message: 'All recovery codes copied', type: 'success' });
                                    }}
                                    className="rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] px-2.5 py-1 text-[11px] font-medium text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)] transition-colors"
                                  >
                                    {t('detail.copy_all_codes')}
                                  </button>
                                </div>
                              </motion.div>
                            )}
                          </div>
                        )}
                      </Fragment>
                    );
                  }
                } else {
                  const cf = displayCustomFields.find(c => c.id === id);
                  if (!cf) return null;
                  const isPassword = cf.type === 'password';
                  const isUrl = cf.type === 'url';
                  const isEmail = cf.type === 'email';
                  const isUsername = cf.type === 'username';

                  let fieldIcon = <FileText size={15} />;
                  if (isPassword) fieldIcon = <Key size={15} />;
                  else if (isEmail) fieldIcon = <Mail size={15} />;
                  else if (isUrl) fieldIcon = <Globe size={15} />;
                  else if (isUsername) fieldIcon = <User size={15} />;

                  return (
                    <motion.div
                      key={cf.id}
                      initial={{ opacity: 0, y: 2 }}
                      animate={{ opacity: 1, y: 0 }}
                      transition={{ duration: 0.1, delay: i * 0.02 }}
                      className="flex items-center gap-3 rounded-[3px] bg-[var(--bg-elevated)] px-3 py-2.5 transition-colors hover:bg-[var(--bg-hover)]"
                    >
                      <span className="shrink-0 text-[var(--text-secondary)]">
                        {fieldIcon}
                      </span>
                      <div className="min-w-0 flex-1">
                        <div className="text-[11px] font-medium uppercase tracking-wide text-[var(--text-tertiary)]">
                          {cf.name || 'Custom Field'}
                        </div>
                        {isEditing && editData ? (
                          <input
                            type={isPassword && !showCustomPasswords[cf.id] ? 'password' : 'text'}
                            value={cf.value}
                            onChange={(e) => {
                              const updatedCustom = editData.customFields.map(f => f.id === cf.id ? { ...f, value: e.target.value } : f);
                              setEditData({ ...editData, customFields: updatedCustom });
                            }}
                            className="w-full rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] px-2 py-0.5 text-[13px] text-[var(--text-primary)] outline-none focus:border-[var(--border-focus)] font-mono"
                          />
                        ) : (
                          <div className={`truncate text-[13px] text-[var(--text-primary)] ${isPassword ? 'font-mono' : ''}`}>
                            {isPassword ? (showCustomPasswords[cf.id] ? cf.value : '••••••••') : cf.value}
                          </div>
                        )}
                      </div>
                      {!isEditing && (
                        <div className="flex items-center gap-1">
                          {isPassword && (
                            <ActionTooltip content={showCustomPasswords[cf.id] ? t('login.hide_password') : t('login.show_password')}>
                              <button
                                type="button"
                                onClick={() => setShowCustomPasswords(prev => ({ ...prev, [cf.id]: !prev[cf.id] }))}
                                className="inline-flex items-center justify-center rounded-[3px] p-1 text-[var(--text-tertiary)] transition-all duration-100 hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)] active:scale-95"
                              >
                                {showCustomPasswords[cf.id] ? <EyeOff size={14} /> : <Eye size={14} />}
                              </button>
                            </ActionTooltip>
                          )}
                          {isUrl && cf.value && (
                            <ActionTooltip content="Open website in browser">
                              <button
                                type="button"
                                onClick={(e) => {
                                  e.stopPropagation();
                                  openExternalUrl(cf.value);
                                }}
                                className="inline-flex items-center justify-center rounded-[3px] p-1 text-[var(--text-tertiary)] transition-all duration-100 hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)] active:scale-95"
                              >
                                <ExternalLink size={14} />
                              </button>
                            </ActionTooltip>
                          )}
                          <AutotypeButton value={cf.value} />
                          <CopyButton value={cf.value} />
                        </div>
                      )}
                    </motion.div>
                  );
                }
              })}
            </div>

            {/* Passkey — only shown when active */}
            {data.hasPasskey && (
              <div className="px-4 py-3 border-t border-[var(--border-subtle)]">
                <div className="flex items-center justify-between">
                  <div className="flex items-center gap-2">
                    <ShieldCheck size={14} className="text-green-500" />
                    <span className="text-[12px] font-medium text-[var(--text-primary)]">
                      Passkey (ES256)
                    </span>
                    {data.passkeyPublicKey && (
                      <span className="text-[10px] font-mono text-[var(--text-tertiary)] truncate max-w-[140px]">
                        {data.passkeyPublicKey.slice(0, 8).map((b: number) => b.toString(16).padStart(2, '0')).join('')}…
                      </span>
                    )}
                  </div>
                  {!isEditing && (
                    <button
                      onClick={async () => {
                        if (!backend || !selectedEntry) return;
                        if (!window.confirm('Remove the passkey from this entry?')) return;
                        try {
                          await backend.updateEntry(selectedEntry.id, { passkey_action: 'remove' } as any);
                          await refreshEntries();
                          await selectEntryById(selectedEntry.id);
                          addToast({ message: 'Passkey removed', type: 'success' });
                        } catch (err) {
                          addToast({ message: `Failed to remove passkey: ${err}`, type: 'error' });
                        }
                      }}
                      className="text-[11px] font-medium text-red-400 hover:text-red-300 transition-colors"
                    >
                      Remove
                    </button>
                  )}
                </div>
              </div>
            )}

            {/* Footer */}
            <div className="mt-auto flex gap-6 border-t border-[var(--border-subtle)] px-4 py-3">
              <span className="text-[12px] text-[var(--text-tertiary)]">
                {t('detail.created')}: {formatDate(data.createdAt)}
              </span>
              <span className="text-[12px] text-[var(--text-tertiary)]">
                {t('detail.updated')}: {formatDate(data.updatedAt)}
              </span>
            </div>
          </motion.div>
        )}
      </AnimatePresence>

      {/* Delete confirmation */}
      <DeleteEntryModal
        entry={showDelConfirm ? selectedEntry : null}
        onClose={() => setShowDelConfirm(false)}
        onConfirm={() => {
          if (selectedEntry) {
            deleteEntry(selectedEntry.id);
          }
        }}
      />

      {/* Smart Login Warning Modal */}
      <AnimatePresence>
        {showWarningModal && (
          <motion.div
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-sm"
            onClick={() => setShowWarningModal(false)}
          >
            <motion.div
              initial={{ scale: 0.96, opacity: 0 }}
              animate={{ scale: 1, opacity: 1 }}
              exit={{ scale: 0.96, opacity: 0 }}
              transition={{ duration: 0.15 }}
              className="relative w-[420px] rounded-[4px] border border-[var(--border)] bg-[var(--bg-base)] p-6 shadow-2xl"
              onClick={(e) => e.stopPropagation()}
            >
              <button
                type="button"
                onClick={() => setShowWarningModal(false)}
                className="absolute top-4 right-4 text-[var(--text-tertiary)] hover:text-[var(--text-primary)] transition-colors p-1 rounded-[3px] hover:bg-[var(--bg-hover)]"
                aria-label="Close dialog"
              >
                <X size={15} />
              </button>

              <div className="flex flex-col">
                <h3 className="text-[16px] font-semibold text-[var(--text-primary)]">
                  Smart Login Information
                </h3>
                <p className="mt-2 text-[12.5px] leading-relaxed text-[var(--text-secondary)]">
                  Smart Login uses OS-level automated typing to fill your credentials directly into the active screen fields.
                </p>
                
                <div className="mt-3.5 space-y-2 rounded-[3px] bg-[var(--bg-elevated)] p-3 text-[11.5px] text-[var(--text-secondary)] border border-[var(--border-subtle)]">
                  <div className="flex gap-2">
                    <span className="text-[var(--text-tertiary)] font-bold">•</span>
                    <span><strong>Active Page:</strong> Keep the target login window or browser tab active on your screen. The fields will be located and filled automatically.</span>
                  </div>
                  <div className="flex gap-2">
                    <span className="text-[var(--text-tertiary)] font-bold">•</span>
                    <span><strong>Safety Guard:</strong> Typing will immediately cancel if you switch active windows to prevent credentials leaking.</span>
                  </div>
                </div>

                <div className="mt-4 flex items-center gap-2">
                  <input
                    type="checkbox"
                    id="dont-show-again"
                    checked={dontShowAgain}
                    onChange={(e) => {
                      const checked = e.target.checked;
                      setDontShowAgain(checked);
                      if (checked) {
                        setWarningTimer(0);
                      }
                    }}
                    className="accent-[var(--accent)] h-3.5 w-3.5 rounded-[3px] border-[var(--border)] cursor-pointer"
                  />
                  <label htmlFor="dont-show-again" className="text-[11.5px] text-[var(--text-secondary)] cursor-pointer select-none">
                    Don't show this warning again
                  </label>
                </div>

                <div className="mt-5 flex justify-end gap-2.5">
                  <button
                    type="button"
                    onClick={() => setShowWarningModal(false)}
                    className="h-9 rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] px-4 text-[13px] font-medium text-[var(--text-secondary)] transition-all hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)] active:scale-[0.98]"
                  >
                    Cancel
                  </button>
                  <button
                    disabled={warningTimer > 0}
                    onClick={() => {
                      if (dontShowAgain) {
                        localStorage.setItem('yntra-vault-skip-smart-login-warning', 'true');
                      }
                      setShowWarningModal(false);
                      runSmartLoginAction();
                    }}
                    className={`h-9 rounded-[3px] px-5 text-[13px] font-medium transition-all ${
                      warningTimer > 0
                        ? 'bg-[var(--border)] text-[var(--text-tertiary)] cursor-not-allowed'
                        : 'bg-[var(--accent)] text-[var(--bg-base)] hover:opacity-90 active:scale-[0.98]'
                    }`}
                  >
                    {warningTimer > 0 ? `I Understand (${warningTimer}s)` : 'I Understand'}
                  </button>
                </div>
              </div>
            </motion.div>
          </motion.div>
        )}
      </AnimatePresence>

      {/* Edit modal */}
      <EntryModal
        open={showEditModal}
        onClose={() => setShowEditModal(false)}
        editEntry={selectedEntry}
      />
    </div>
  );
}

function RecoveryCodeItem({ code, index, onCopy }: { code: string; index: number; onCopy: () => void }) {
  const [hovered, setHovered] = useState(false);
  const [copied, setCopied] = useState(false);

  const handleCopy = () => {
    navigator.clipboard.writeText(code);
    setCopied(true);
    onCopy();
    setTimeout(() => setCopied(false), 1500);
  };

  return (
    <ActionTooltip content={copied ? 'Copied code!' : 'Click to copy recovery code'}>
      <button
        onClick={handleCopy}
        onMouseEnter={() => setHovered(true)}
        onMouseLeave={() => setHovered(false)}
        className="flex items-center justify-between rounded-[3px] border border-[var(--border-subtle)] bg-[var(--bg-elevated)] px-2.5 py-1.5 text-left transition-all hover:border-[var(--border-focus)] hover:bg-[var(--bg-hover)] active:scale-[0.98] group cursor-pointer w-full outline-none"
      >
        <span className="truncate text-[var(--text-primary)] font-medium font-mono tracking-wide">
          {hovered ? code : '••••••••'}
        </span>
        <span className="text-[9px] text-[var(--text-tertiary)] shrink-0 select-none group-hover:text-[var(--text-secondary)] transition-colors ml-2">
          {copied ? 'Copied' : `#${index + 1}`}
        </span>
      </button>
    </ActionTooltip>
  );
}

function TOTPField({ secret, index }: { secret: string; index: number }) {
  const code = useTotp(secret);
  const isUrgent = code ? code.seconds_remaining <= 5 : false;

  // Format code with space in middle: "123 456"
  const formattedCode = code
    ? (code.code.length === 6 ? `${code.code.slice(0, 3)} ${code.code.slice(3)}` : code.code)
    : 'Generating...';

  const progress = code ? code.seconds_remaining / code.period : 1;

  return (
    <motion.div
      initial={{ opacity: 0, y: 2 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ duration: 0.1, delay: index * 0.02 }}
      className="flex items-center gap-3 rounded-[3px] bg-[var(--bg-elevated)] px-3 py-2.5 transition-colors hover:bg-[var(--bg-hover)] mt-4"
    >
      <span className="shrink-0 text-[var(--text-secondary)]">
        <ShieldCheck size={15} />
      </span>
      <div className="min-w-0 flex-1">
        <div className="text-[11px] font-medium uppercase tracking-wide text-[var(--text-tertiary)] flex items-center gap-1.5">
          <span>2FA Code</span>
          {code && (
            <CountdownRing progress={progress} size={10} urgent={isUrgent} />
          )}
        </div>
        <div className="flex items-baseline gap-1.5">
          <span className={`font-mono text-[13px] font-semibold tracking-wider ${
            isUrgent ? 'text-red-400 animate-pulse' : 'text-[var(--text-primary)]'
          }`}>
            {formattedCode}
          </span>
          {code && (
            <span className="text-[10px] text-[var(--text-tertiary)] select-none">
              ({code.seconds_remaining}s)
            </span>
          )}
        </div>
      </div>
      {code && (
        <div className="flex items-center gap-1">
          <AutotypeButton value={code.code} />
          <CopyButton value={code.code} />
        </div>
      )}
    </motion.div>
  );
}

const CountdownRing: React.FC<{
  progress: number;
  size: number;
  urgent: boolean;
}> = ({ progress, size, urgent }) => {
  const r = size / 2 - 1.5;
  const circumference = 2 * Math.PI * r;
  const strokeDashoffset = circumference * (1 - progress);
  const color = urgent ? '#ef4444' : 'var(--accent, #3b82f6)';

  return (
    <svg width={size} height={size} className="shrink-0 -rotate-90">
      <circle
        cx={size / 2}
        cy={size / 2}
        r={r}
        fill="none"
        stroke="var(--bg-hover)"
        strokeWidth={1.5}
      />
      <circle
        cx={size / 2}
        cy={size / 2}
        r={r}
        fill="none"
        stroke={color}
        strokeWidth={1.5}
        strokeDasharray={circumference}
        strokeDashoffset={strokeDashoffset}
        strokeLinecap="round"
        className="transition-all duration-1000 linear"
      />
    </svg>
  );
};

const PasswordSafetySection: React.FC<{
  password?: string;
  status?: BreachStatus;
  showTemporaryStats: boolean;
}> = ({ password, status, showTemporaryStats }) => {
  const [breachStatus, setBreachStatus] = useState<BreachStatus | null>(status || null);
  const { selectedEntry, refreshEntries } = useAppState();
  const { backend } = useBackend();

  useEffect(() => {
    if (status) {
      setBreachStatus(status);
    }
  }, [status]);

  if (!password) return null;

  const isSafe = breachStatus?.type === 'Safe' || breachStatus?.type === 'Unknown';
  const shouldShowContainer = showTemporaryStats || !isSafe;

  if (!shouldShowContainer) return null;

  return (
    <motion.div
      initial={{ opacity: 0, y: 2 }}
      animate={{ opacity: 1, y: 0 }}
      exit={{ opacity: 0, y: -2 }}
      className="flex flex-col gap-2 rounded-[3px] bg-[var(--bg-elevated)] px-3 py-2.5 mt-2"
    >
      {showTemporaryStats && <PasswordStrength password={password} />}
      <BreachIndicator
        password={password}
        status={status}
        onStatusChange={async (newStatus) => {
          setBreachStatus(newStatus);
          if (selectedEntry && backend) {
            try {
              await backend.updateEntry(selectedEntry.id, { breach_status: newStatus } as any);
              await refreshEntries();
            } catch (err) {
              console.error('Failed to save manual breach status check:', err);
            }
          }
        }}
        hideIfSafe={!showTemporaryStats}
      />
    </motion.div>
  );
};



