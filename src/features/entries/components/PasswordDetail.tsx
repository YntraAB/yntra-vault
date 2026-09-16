import { useState, useEffect, useCallback, useMemo, useRef } from 'react';
import { motion } from 'framer-motion';
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
  Paperclip,
  Download,
  Loader2,
  Play,
  MoreVertical,
} from 'lucide-react';
import { useEntries, isRecoveryField } from '../context/EntriesContext';
import { useSettings } from '@/features/settings';
import { useToast } from '@/contexts/ToastContext';
import { useUi } from '@/contexts/UiContext';
import { useTranslation } from '@/contexts/LanguageContext';
import { getBackend, isTauri } from '@/lib/backend';
import { CopyButton, PasswordInput, Skeleton } from '@/components/ui';
import {
  DropdownMenu,
  DropdownMenuTrigger,
  DropdownMenuContent,
  DropdownMenuItem,
} from '@/components/ui/dropdown-menu';
import { AutotypeButton } from './AutotypeButton';
import SmartLoginButton from './SmartLoginButton';
import { PasswordStrength, BreachIndicator, useSecurityAudit } from '@/features/audit';
import { DeleteEntryModal } from './DeleteEntryModal';
import type { BreachStatus } from '@/lib/backend';
import { useTotp } from '../hooks/useTotp';
import { useBackend } from '@/lib/useBackend';
import { AttachmentPreviewModal } from './AttachmentPreviewModal';
import { Favicon } from './Favicon';
import { formatDate, getFieldLayout, openExternalUrl } from '@/lib/utils';
import { formatBytes, getAttachmentIcon } from '@/lib/formatters';
import type { Tag, AttachmentInfo } from '@/types';
import { ActionTooltip } from '@/components/ui/tooltip';
import { matchesShortcut, getKeybinds } from '@/lib/keybinds';

export function PasswordDetail() {
  const { t } = useTranslation();
  const {
    selectedEntry,
    deleteEntry,
    deleteAttachment,
    updateEntry,
    tags,
    togglePin,
    toggleFavorite,
    isLoadingDetail,
  } = useEntries();
  const {
    isEditing,
    setIsEditing,
    settingsOpen,
    isEntryModalOpen,
    openEditModal,
    setFilterCategory,
  } = useUi();
  const { settings } = useSettings();
  const { addToast } = useToast();
  const { backend } = useBackend();
  const [editData, setEditData] = useState(selectedEntry);
  const [showDelConfirm, setShowDelConfirm] = useState(false);
  const [showRecovery, setShowRecovery] = useState(false);
  const [showTemporaryStats, setShowTemporaryStats] = useState(false);
  const [showPassword, setShowPassword] = useState(false);
  const [showCustomPasswords, setShowCustomPasswords] = useState<Record<string, boolean>>({});
  const [downloadingAttId, setDownloadingAttId] = useState<string | null>(null);
  const [previewAtt, setPreviewAtt] = useState<AttachmentInfo | null>(null);
  const [previewData, setPreviewData] = useState<Uint8Array | null>(null);
  const [loadingPreviewId, setLoadingPreviewId] = useState<string | null>(null);
  const previewCacheRef = useRef<Map<string, Uint8Array>>(new Map());
  const activePreviewReqIdRef = useRef<string | null>(null);

  // Wipe and clear preview cache on entry change or unmount
  useEffect(() => {
    return () => {
      for (const buf of previewCacheRef.current.values()) {
        buf.fill(0);
      }
      previewCacheRef.current.clear();
    };
  }, [selectedEntry?.id]);

  const handlePrefetchAttachment = useCallback((attachment: AttachmentInfo) => {
    if (!backend || !selectedEntry) return;
    if (attachment.size > 256 * 1024) return;
    if (previewCacheRef.current.has(attachment.id)) return;
    backend.getAttachmentData(selectedEntry.id, attachment.id).then((raw) => {
      const uint8 = raw instanceof Uint8Array ? raw : new Uint8Array(raw);
      previewCacheRef.current.set(attachment.id, uint8);
    }).catch(() => {});
  }, [backend, selectedEntry]);

  const handlePreviewAttachment = useCallback(async (attachment: AttachmentInfo) => {
    if (!backend || !selectedEntry) return;

    activePreviewReqIdRef.current = attachment.id;

    // Fast path: cached attachment opens immediately without any loading state
    const cached = previewCacheRef.current.get(attachment.id);
    if (cached) {
      setPreviewAtt(attachment);
      setPreviewData(cached);
      return;
    }

    // Immediately open modal shell with in-modal loader
    setPreviewAtt(attachment);
    setPreviewData(null);
    setLoadingPreviewId(attachment.id);

    try {
      const raw = await backend.getAttachmentData(selectedEntry.id, attachment.id);

      // If user closed the modal while data was fetching, abort immediately
      if (activePreviewReqIdRef.current !== attachment.id) {
        return;
      }

      const uint8 = raw instanceof Uint8Array ? raw : new Uint8Array(raw);
      previewCacheRef.current.set(attachment.id, uint8);
      setPreviewData(uint8);
    } catch (err) {
      if (activePreviewReqIdRef.current === attachment.id) {
        addToast({ message: t('toast.load_preview_failed', { err: String(err) }), type: 'error' });
        setPreviewAtt(null);
      }
    } finally {
      if (activePreviewReqIdRef.current === attachment.id) {
        setLoadingPreviewId(null);
      }
    }
  }, [backend, selectedEntry, addToast, t]);

  const handleClosePreview = useCallback(() => {
    activePreviewReqIdRef.current = null;
    setLoadingPreviewId(null);
    setPreviewAtt(null);
    setPreviewData(null);
  }, []);

  const handleDownloadAttachment = useCallback(async (attachment: AttachmentInfo) => {
    if (!backend || !selectedEntry) return;
    setDownloadingAttId(attachment.id);
    try {
      const cached = previewCacheRef.current.get(attachment.id);
      let uint8: Uint8Array;
      let shouldZero = false;

      if (cached) {
        uint8 = cached;
      } else {
        const raw = await backend.getAttachmentData(selectedEntry.id, attachment.id);
        uint8 = raw instanceof Uint8Array ? raw : new Uint8Array(raw);
        shouldZero = true;
      }

      const blob = new Blob([uint8 as Uint8Array<ArrayBuffer>], { type: attachment.mime_type || 'application/octet-stream' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = attachment.name;
      document.body.appendChild(a);
      a.click();
      document.body.removeChild(a);
      URL.revokeObjectURL(url);
      if (shouldZero) {
        uint8.fill(0);
      }
      addToast({ message: t('toast.downloaded_attachment', { name: attachment.name }), type: 'success' });
    } catch (err) {
      addToast({ message: t('toast.download_failed', { err: String(err) }), type: 'error' });
    } finally {
      setDownloadingAttId(null);
    }
  }, [backend, selectedEntry, addToast, t]);

  const handleDeleteAttachment = useCallback(async (attachment: AttachmentInfo) => {
    if (!selectedEntry) return;
    if (!window.confirm(`Delete attachment "${attachment.name}"?`)) return;
    try {
      await deleteAttachment(selectedEntry.id, attachment.id);
      const cached = previewCacheRef.current.get(attachment.id);
      if (cached) {
        cached.fill(0);
        previewCacheRef.current.delete(attachment.id);
      }
      addToast({ message: t('toast.attachment_deleted'), type: 'success' });
    } catch (err) {
      addToast({ message: t('toast.delete_attachment_failed', { err: String(err) }), type: 'error' });
    }
  }, [selectedEntry, deleteAttachment, addToast, t]);

  // Keyboard shortcuts to copy entry details (Password, Username, URL, TOTP) using single configured keybinds
  useEffect(() => {
    if (!selectedEntry || isEditing) return;

    const handleCopyShortcuts = async (e: KeyboardEvent) => {
      if (settingsOpen || isEntryModalOpen || showDelConfirm || previewAtt) {
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
          if (isTauri() && backend) {
            backend.copyEntryPassword(selectedEntry.id, settings.clipboardClearSeconds).catch(() => {});
          } else if (backend) {
            backend.copyToClipboard(selectedEntry.password, true, settings.clipboardClearSeconds).catch(() => {});
          } else {
            navigator.clipboard.writeText(selectedEntry.password).catch(() => {});
          }
          addToast({ message: t('toast.copied_password'), type: 'info' });
        }
        return;
      }

      // 2. Copy Username
      if (matchesShortcut(e, kb.copyUsername)) {
        if (selectedEntry.username) {
          e.preventDefault();
          e.stopPropagation();
          if (isTauri() && backend) {
            backend.copyEntryUsername(selectedEntry.id).catch(() => {});
          } else if (backend) {
            backend.copyToClipboard(selectedEntry.username, false).catch(() => {});
          } else {
            navigator.clipboard.writeText(selectedEntry.username).catch(() => {});
          }
          addToast({ message: t('toast.copied_username'), type: 'info' });
        }
        return;
      }

      // 3. Copy Website URL
      if (matchesShortcut(e, kb.copyUrl)) {
        if (selectedEntry.url) {
          e.preventDefault();
          e.stopPropagation();
          if (backend) {
            backend.copyToClipboard(selectedEntry.url, false).catch(() => {});
          } else {
            navigator.clipboard.writeText(selectedEntry.url).catch(() => {});
          }
          addToast({ message: t('toast.copied_url'), type: 'info' });
        }
        return;
      }

      // 4. Copy TOTP / 2FA Code
      if (matchesShortcut(e, kb.copyTotp)) {
        if (selectedEntry.totpSecret) {
          e.preventDefault();
          e.stopPropagation();
          if (isTauri() && backend) {
            backend.copyEntryTotp(selectedEntry.id, settings.clipboardClearSeconds)
              .then(() => addToast({ message: t('toast.copied_totp'), type: 'info' }))
              .catch(() => addToast({ message: t('toast.totp_failed'), type: 'error' }));
          } else if (backend) {
            try {
              const totpRes = await backend.generateTotp(selectedEntry.totpSecret);
              if (totpRes && totpRes.code) {
                await backend.copyToClipboard(totpRes.code, true, settings.clipboardClearSeconds);
                addToast({ message: t('toast.copied_totp'), type: 'info' });
              }
            } catch {
              addToast({ message: t('toast.totp_failed'), type: 'error' });
            }
          }
        }
        return;
      }

      // 5. Edit Entry
      if (matchesShortcut(e, kb.editEntry)) {
        e.preventDefault();
        e.stopPropagation();
        openEditModal(selectedEntry);
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
    openEditModal,
    previewAtt,
    addToast,
    backend,
  ]);

  useEffect(() => {
    if (!showDelConfirm) return;
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        setShowDelConfirm(false);
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [showDelConfirm]);

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

  const data = isEditing && editData && editData.id === selectedEntry?.id ? editData : selectedEntry;
  const entryTags = data
    ? (data.tags.map((t) => tags.find((tag) => tag.name === t)).filter(Boolean) as Tag[])
    : [];

  const activeStandard: string[] = [];
  if (data) {
    const layoutCf = data.customFields.find(cf => cf.name === '_field_order');
    if (layoutCf && layoutCf.value) {
      const savedOrder = layoutCf.value.split(',').map(s => s.trim()).filter(Boolean);
      savedOrder.forEach(f => {
        if (['username', 'password', 'url', 'email', 'notes', 'totpSecret'].includes(f)) {
          if (isEditing) {
            activeStandard.push(f);
          } else {
            if (f === 'username' && data.username?.trim()) activeStandard.push(f);
            else if (f === 'password' && data.password && data.password !== '••••••••') activeStandard.push(f);
            else if (f === 'url' && data.url?.trim()) activeStandard.push(f);
            else if (f === 'email' && data.email?.trim()) activeStandard.push(f);
            else if (f === 'notes' && data.notes?.trim()) activeStandard.push(f);
            else if (f === 'totpSecret' && data.totpSecret && data.totpSecret !== 'has-totp') activeStandard.push(f);
          }
        }
      });
    } else {
      if (data.username?.trim()) activeStandard.push('username');
      if (data.password && data.password !== '••••••••') activeStandard.push('password');
      if (data.url?.trim()) activeStandard.push('url');
      if (data.email?.trim()) activeStandard.push('email');
      if (data.notes?.trim()) activeStandard.push('notes');
      if (data.totpSecret && data.totpSecret !== 'has-totp') activeStandard.push('totpSecret');
    }
  }

  const effectiveRecoveryCodes = useMemo(() => {
    if (!data) return undefined;
    if (data.recoveryCodes && data.recoveryCodes.trim()) {
      return data.recoveryCodes;
    }
    const legacy = data.customFields
      .filter((cf) => isRecoveryField(cf.name) && cf.value && cf.value.trim())
      .map((cf) => cf.value.trim())
      .join('\n');
    return legacy || undefined;
  }, [data]);

  const displayCustomFields = data
    ? data.customFields.filter(
        (cf) =>
          cf.name !== '_field_order' &&
          !isRecoveryField(cf.name) &&
          (isEditing || (cf.value && cf.value.trim() !== ''))
      )
    : [];
  const layoutOrder = data ? getFieldLayout(displayCustomFields, activeStandard) : [];

  const handleCopyTitle = useCallback(async () => {
    if (!data?.title) return;
    try {
      if (isTauri()) {
        const backend = await getBackend();
        await backend.copyToClipboard(data.title, false);
      } else {
        await navigator.clipboard.writeText(data.title);
      }
      addToast({ message: t('detail.copied_title', { title: data.title }), type: 'info' });
    } catch {
      addToast({ message: t('toast.copied_title_failed'), type: 'error' });
    }
  }, [data?.title, addToast, t]);

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
      {isLoadingDetail ? (
        <div className="flex flex-col">
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
        </div>
      ) : !selectedEntry ? (
        <div className="flex h-[80vh] flex-col items-center justify-center select-none">
          <div className="text-center">
            <p className="text-[16px] font-semibold text-[var(--text-tertiary)]">{t('detail.select_entry')}</p>
            <p className="mt-1 text-[13px] text-[var(--text-tertiary)]">
              {t('detail.select_entry_desc')}
            </p>
          </div>
        </div>
      ) : !data ? null : (
        <div className="flex flex-col">
            {/* Header */}
            <div className="flex flex-col sm:flex-row sm:items-start justify-between gap-3 border-b border-[var(--border-subtle)] p-3.5 sm:p-4 select-none">
              <div className="flex items-start gap-3 min-w-0 flex-1">
                {/* Favicon with Tooltip */}
                <ActionTooltip content={data.url ? t('detail.favicon_url_tooltip', { domain: data.url }) : t('detail.favicon_category_tooltip', { title: data.title })}>
                  <div className="shrink-0">
                    <Favicon
                      url={data.url}
                      title={data.title}
                      color={entryTags[0]?.color}
                      sizeClass="h-9 w-9"
                      textClass="text-[12px]"
                    />
                  </div>
                </ActionTooltip>

                <div className="flex flex-col min-w-0 flex-1">
                  {isEditing && editData ? (
                    <input
                      type="text"
                      value={editData.title}
                      onChange={(e) => setEditData({ ...editData, title: e.target.value })}
                      className="w-full rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] px-2 py-1 text-[18px] sm:text-[20px] font-semibold outline-none focus:border-[var(--border-focus)]"
                    />
                  ) : (
                    <div className="min-w-0 max-w-full">
                      <ActionTooltip content={t('detail.copy_title_tooltip')}>
                        <h1
                          onClick={handleCopyTitle}
                          className="text-[18px] sm:text-[20px] font-semibold leading-tight tracking-tight text-[var(--text-primary)] truncate max-w-full cursor-pointer select-text hover:text-[var(--text-secondary)] transition-colors inline-block"
                        >
                          {data.title}
                        </h1>
                      </ActionTooltip>
                    </div>
                  )}
                  {data.url && !isEditing && (
                    <div className="min-w-0 max-w-full">
                      <a
                        href={/^https?:\/\//i.test(data.url) ? data.url : `https://${data.url}`}
                        target="_blank"
                        rel="noopener noreferrer"
                        onClick={(e) => {
                          e.preventDefault();
                          e.stopPropagation();
                          openExternalUrl(data.url);
                        }}
                        className="mt-0.5 inline-block truncate text-[12px] text-[var(--text-secondary)] transition-colors hover:text-[var(--text-primary)] max-w-full select-text"
                      >
                        {data.url}
                      </a>
                    </div>
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
              <div className="flex items-center justify-end sm:justify-start gap-1.5 shrink-0 pt-2.5 sm:pt-0 border-t sm:border-t-0 border-[var(--border-subtle)]/40 w-full sm:w-auto">
                {isEditing ? (
                  <div className="flex items-center gap-2 w-full sm:w-auto">
                    <button
                      onClick={handleSave}
                      className="flex-1 sm:flex-initial h-8.5 rounded-[3px] bg-[var(--text-primary)] px-4 text-[13px] font-medium text-[var(--bg-base)] transition-colors hover:bg-[var(--accent-hover)] cursor-pointer"
                    >
                      {t('common.save')}
                    </button>
                    <button
                      onClick={() => setIsEditing(false)}
                      className="flex-1 sm:flex-initial h-8.5 rounded-[3px] px-4 text-[13px] font-medium text-[var(--text-secondary)] transition-colors hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)] cursor-pointer"
                    >
                      {t('common.cancel')}
                    </button>
                  </div>
                ) : (
                  <>
                    {/* Pin toggle */}
                    <ActionTooltip content={data.pinned ? t('menu.unpin') : t('menu.pin')}>
                      <button
                        onClick={() => togglePin(data.id)}
                        className={`inline-flex h-8.5 w-8.5 items-center justify-center rounded-[3px] transition-colors shrink-0 cursor-pointer ${data.pinned
                          ? 'text-yellow-500 hover:bg-yellow-500/10'
                          : 'text-[var(--text-tertiary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]'
                          }`}
                      >
                        <Pin size={16} className={data.pinned ? 'fill-current' : ''} />
                      </button>
                    </ActionTooltip>

                    {/* Favorite toggle */}
                    <ActionTooltip content={data.favorite ? t('detail.fav_remove') : t('detail.fav_add')}>
                      <button
                        onClick={() => toggleFavorite(data.id)}
                        className={`inline-flex h-8.5 w-8.5 items-center justify-center rounded-[3px] transition-colors shrink-0 cursor-pointer ${data.favorite
                          ? 'text-orange-500 hover:bg-orange-500/10'
                          : 'text-[var(--text-tertiary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]'
                          }`}
                      >
                        <Star size={16} className={data.favorite ? 'fill-current' : ''} />
                      </button>
                    </ActionTooltip>

                    {/* Smart Login */}
                    {isTauri() && !!data.url && (
                      <SmartLoginButton
                        entryId={data.id}
                        entryTitle={data.title}
                        hasUrl={!!data.url}
                      />
                    )}

                    <ActionTooltip content={t('common.edit')}>
                      <button
                        onClick={() => selectedEntry && openEditModal(selectedEntry)}
                        className="inline-flex h-8.5 items-center justify-center gap-1.5 rounded-[3px] px-3 text-[13px] font-medium text-[var(--text-secondary)] transition-colors hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)] shrink-0 cursor-pointer"
                      >
                        <Pencil size={15} className="shrink-0" />
                        <span className="hidden sm:inline">{t('common.edit')}</span>
                      </button>
                    </ActionTooltip>

                    <ActionTooltip content={t('common.delete')}>
                      <button
                        onClick={() => setShowDelConfirm(true)}
                        className="inline-flex h-8.5 items-center justify-center gap-1.5 rounded-[3px] px-3 text-[13px] font-medium text-[var(--destructive)] transition-colors hover:bg-[var(--destructive)]/8 shrink-0 cursor-pointer"
                      >
                        <Trash2 size={15} className="shrink-0" />
                        <span className="hidden sm:inline">{t('common.delete')}</span>
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
                        transition={{ duration: 0.05 }}
                        className={`flex items-center gap-3 rounded-[3px] bg-[var(--bg-elevated)] ${fieldItemPaddingClass} transition-colors hover:bg-[var(--bg-hover)]`}
                      >
                        <span className="shrink-0 text-[var(--text-secondary)] select-none">
                          <User size={15} />
                        </span>
                        <div className="min-w-0 flex-1">
                          <div className="text-[11px] font-medium uppercase tracking-wide text-[var(--text-tertiary)] select-none">
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
                            <div className="truncate text-[13px] text-[var(--text-primary)] select-text">
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
                      <div key="password-group" className="flex flex-col gap-1">
                        <motion.div
                          initial={{ opacity: 0, y: 2 }}
                          animate={{ opacity: 1, y: 0 }}
                          transition={{ duration: 0.05 }}
                          className={`flex items-center gap-3 rounded-[3px] bg-[var(--bg-elevated)] ${fieldItemPaddingClass} transition-colors hover:bg-[var(--bg-hover)]`}
                        >
                          <span className="shrink-0 text-[var(--text-secondary)] select-none">
                            <Key size={15} />
                          </span>
                          <div className="min-w-0 flex-1">
                            <div className="text-[11px] font-medium uppercase tracking-wide text-[var(--text-tertiary)] select-none">
                              {t('detail.password')}
                            </div>
                            {isEditing && editData ? (
                              <PasswordInput
                                value={editData.password}
                                onChange={(v) => setEditData({ ...editData, password: v })}
                              />
                            ) : (
                              <span className={`font-mono text-[13px] tracking-wider text-[var(--text-primary)] ${showPassword ? 'select-text' : 'select-none'}`}>
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
                                  className="inline-flex items-center justify-center rounded-[3px] p-1.5 sm:p-1 text-[var(--text-tertiary)] transition-colors hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)] cursor-pointer"
                                >
                                  {showPassword ? <EyeOff size={14} /> : <Eye size={14} />}
                                </button>
                              </ActionTooltip>
                              <AutotypeButton value={data.password} entryId={selectedEntry.id} />
                              <CopyButton value={data.password} />
                            </div>
                          )}
                        </motion.div>
                        {data.password && !isEditing && (
                          <PasswordSafetySection
                            password={data.password}
                            status={data.breachStatus}
                            showTemporaryStats={showTemporaryStats}
                            entryId={selectedEntry.id}
                          />
                        )}
                      </div>
                    );
                  }

                  if (id === 'url') {
                    const isAppPath = /[\\/]|\.exe$|\.app$/i.test(data.url);
                    return (
                      <motion.div
                        key="url"
                        initial={{ opacity: 0, y: 2 }}
                        animate={{ opacity: 1, y: 0 }}
                        transition={{ duration: 0.05 }}
                        className={`flex items-center gap-3 rounded-[3px] bg-[var(--bg-elevated)] ${fieldItemPaddingClass} transition-colors hover:bg-[var(--bg-hover)]`}
                      >
                        <span className="shrink-0 text-[var(--text-secondary)] select-none">
                          <Link size={15} />
                        </span>
                        <div className="min-w-0 flex-1">
                          <div className="text-[11px] font-medium uppercase tracking-wide text-[var(--text-tertiary)] select-none">
                            {t('detail.url')}
                          </div>
                          {isEditing && editData ? (
                            <input
                              type="text"
                              value={editData.url}
                              onChange={(e) => setEditData({ ...editData, url: e.target.value })}
                              placeholder={t('entry_modal.url_app_placeholder')}
                              className="w-full rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] px-2 py-0.5 text-[13px] text-[var(--text-primary)] outline-none focus:border-[var(--border-focus)] font-mono text-[12px]"
                            />
                          ) : (
                            <div className="truncate text-[13px] text-[var(--text-primary)] font-mono text-[12px] select-text">
                              {data.url}
                            </div>
                          )}
                        </div>
                        {!isEditing && (
                          <div className="flex items-center gap-1">
                            {data.url && (
                              <ActionTooltip content={isAppPath ? 'Launch Application' : t('detail.open_website')}>
                                <button
                                  type="button"
                                  onClick={(e) => {
                                    e.preventDefault();
                                    e.stopPropagation();
                                    openExternalUrl(data.url);
                                  }}
                                  className="inline-flex items-center justify-center rounded-[3px] p-1.5 sm:p-1 text-[var(--text-tertiary)] transition-all duration-100 hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)] active:scale-95 cursor-pointer"
                                >
                                  {isAppPath ? <Play size={14} /> : <ExternalLink size={14} />}
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
                        transition={{ duration: 0.05 }}
                        className={`flex items-center gap-3 rounded-[3px] bg-[var(--bg-elevated)] ${fieldItemPaddingClass} transition-colors hover:bg-[var(--bg-hover)]`}
                      >
                        <span className="shrink-0 text-[var(--text-secondary)] select-none">
                          <Mail size={15} />
                        </span>
                        <div className="min-w-0 flex-1">
                          <div className="text-[11px] font-medium uppercase tracking-wide text-[var(--text-tertiary)] select-none">
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
                            <div className="truncate text-[13px] text-[var(--text-primary)] select-text">
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
                        transition={{ duration: 0.05 }}
                        className={`flex items-start gap-3 rounded-[3px] bg-[var(--bg-elevated)] ${fieldItemPaddingClass} transition-colors hover:bg-[var(--bg-hover)]`}
                      >
                        <span className="mt-0.5 shrink-0 text-[var(--text-secondary)] select-none">
                          <FileText size={15} />
                        </span>
                        <div className="min-w-0 flex-1">
                          <div className="text-[11px] font-medium uppercase tracking-wide text-[var(--text-tertiary)] select-none">
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
                            <p className="whitespace-pre-wrap text-[13px] leading-relaxed text-[var(--text-primary)] select-text">
                              {data.notes}
                            </p>
                          )}
                        </div>
                        {!isEditing && data.notes && (
                          <div className="shrink-0 mt-0.5">
                            <CopyButton value={data.notes} />
                          </div>
                        )}
                      </motion.div>
                    );
                  }

                  if (id === 'totpSecret') {
                    return (
                      <TOTPField
                        key="totpSecret"
                        secret={data.totpSecret || ''}
                        index={i}
                      >
                        {effectiveRecoveryCodes && (
                          <RecoveryCodesCard
                            codes={effectiveRecoveryCodes}
                            showRecovery={showRecovery}
                            setShowRecovery={setShowRecovery}
                          />
                        )}
                      </TOTPField>
                    );
                  }
                } else {
                  const cf = displayCustomFields.find(c => c.id === id);
                  if (!cf) return null;

                  if (cf.type === 'totp' && cf.value && !isEditing) {
                    return (
                      <TOTPField
                        key={cf.id}
                        secret={cf.value}
                        index={i}
                        label={cf.name || t('detail.totp')}
                      />
                    );
                  }

                  const isPassword = cf.type === 'password';
                  const isUrl = cf.type === 'url';
                  const isEmail = cf.type === 'email';
                  const isUsername = cf.type === 'username';
                  const isTotp = cf.type === 'totp';

                  let fieldIcon = <FileText size={15} />;
                  if (isPassword) fieldIcon = <Key size={15} />;
                  else if (isEmail) fieldIcon = <Mail size={15} />;
                  else if (isUrl) fieldIcon = <Globe size={15} />;
                  else if (isUsername) fieldIcon = <User size={15} />;
                  else if (isTotp) fieldIcon = <ShieldCheck size={15} />;

                  return (
                    <motion.div
                      key={cf.id}
                      initial={{ opacity: 0, y: 2 }}
                      animate={{ opacity: 1, y: 0 }}
                      transition={{ duration: 0.1, delay: i * 0.02 }}
                      className="flex items-center gap-3 rounded-[3px] bg-[var(--bg-elevated)] px-3 py-2.5 transition-colors hover:bg-[var(--bg-hover)]"
                    >
                      <span className="shrink-0 text-[var(--text-secondary)] select-none">
                        {fieldIcon}
                      </span>
                      <div className="min-w-0 flex-1">
                        <div className="text-[11px] font-medium uppercase tracking-wide text-[var(--text-tertiary)] select-none">
                          {cf.name || t('entry.custom_field')}
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
                          <div className={`truncate text-[13px] text-[var(--text-primary)] ${isPassword ? 'font-mono' : ''} ${isPassword && !showCustomPasswords[cf.id] ? 'select-none' : 'select-text'}`}>
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
                                className="inline-flex items-center justify-center rounded-[3px] p-1.5 sm:p-1 text-[var(--text-tertiary)] transition-all duration-100 hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)] active:scale-95 cursor-pointer"
                              >
                                {showCustomPasswords[cf.id] ? <EyeOff size={14} /> : <Eye size={14} />}
                              </button>
                            </ActionTooltip>
                          )}
                          {isUrl && cf.value && (
                            <ActionTooltip content={t('detail.open_website')}>
                              <button
                                type="button"
                                onClick={(e) => {
                                  e.stopPropagation();
                                  openExternalUrl(cf.value);
                                }}
                                className="inline-flex items-center justify-center rounded-[3px] p-1.5 sm:p-1 text-[var(--text-tertiary)] transition-all duration-100 hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)] active:scale-95 cursor-pointer"
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

              {!layoutOrder.includes('totpSecret') && effectiveRecoveryCodes && (
                <div className="rounded-[3px] bg-[var(--bg-elevated)] px-3 py-2.5">
                  <RecoveryCodesCard
                    codes={effectiveRecoveryCodes}
                    showRecovery={showRecovery}
                    setShowRecovery={setShowRecovery}
                    noBorder
                  />
                </div>
              )}
            </div>

            {/* Passkey — only shown when active */}
            {data.hasPasskey && (
              <div className="px-4 py-3 border-t border-[var(--border-subtle)] select-none">
                <div className="flex items-center justify-between">
                  <div className="flex items-center gap-2">
                    <ShieldCheck size={14} className="text-[var(--text-primary)]" />
                    <span className="text-[12px] font-medium text-[var(--text-primary)]">
                      Passkey (ES256)
                    </span>
                    {data.passkeyPublicKey && (
                      <span className="text-[10px] font-mono text-[var(--text-tertiary)] truncate max-w-[140px] select-text">
                        {data.passkeyPublicKey.slice(0, 8).map((b: number) => b.toString(16).padStart(2, '0')).join('')}…
                      </span>
                    )}
                  </div>
                  {!isEditing && (
                    <button
                      onClick={async () => {
                        if (!selectedEntry) return;
                        if (!window.confirm('Remove the passkey from this entry?')) return;
                        try {
                          await updateEntry({ ...selectedEntry, passkeyAction: 'remove' });
                          addToast({ message: t('toast.passkey_removed'), type: 'success' });
                        } catch (err) {
                          addToast({ message: t('toast.remove_passkey_failed', { err: String(err) }), type: 'error' });
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

            {/* Attachments section */}
            {data.attachments && data.attachments.length > 0 && (
              <div className="px-4 py-3 border-t border-[var(--border-subtle)]">
                <div className="flex items-center gap-1.5 text-[12px] font-medium text-[var(--text-secondary)] mb-2 select-none">
                  <Paperclip size={13} /> Attachments
                  <span className="rounded-full bg-[var(--bg-elevated)] px-2 py-0.5 text-[10px] font-semibold text-[var(--text-secondary)] border border-[var(--border)]">
                    {data.attachments.length}
                  </span>
                </div>
                <div className="flex flex-col gap-2">
                  {data.attachments.map((att: AttachmentInfo) => (
                    <div
                      key={att.id}
                      className="flex items-center justify-between rounded-md border border-[var(--border-subtle)] bg-[var(--bg-elevated)] px-3 py-2.5 shadow-sm hover:border-[var(--border)] transition-colors"
                    >
                      <div
                        onClick={() => handlePreviewAttachment(att)}
                        onMouseEnter={() => handlePrefetchAttachment(att)}
                        className="flex items-center gap-2.5 min-w-0 flex-1 pr-2 cursor-pointer group/att"
                      >
                        <span className="shrink-0 select-none">{getAttachmentIcon(att.mime_type || att.mimeType, att.name)}</span>
                        <div className="flex flex-col min-w-0">
                          <span className="truncate text-[12.5px] font-medium text-[var(--text-primary)] group-hover/att:text-[var(--accent-hover)] transition-colors select-text">
                            {att.name}
                          </span>
                          <span className="text-[10.5px] text-[var(--text-tertiary)] font-mono select-none">
                            {formatBytes(att.size)}
                          </span>
                        </div>
                      </div>

                      <div className="flex items-center gap-1 shrink-0">
                        {!isEditing && (
                          <ActionTooltip content={t('entry.remove_attachment')}>
                            <button
                              type="button"
                              onClick={() => handleDeleteAttachment(att)}
                              className="rounded-md p-1.5 text-[var(--text-tertiary)] hover:bg-[var(--bg-hover)] hover:text-red-400 transition-colors"
                            >
                              <Trash2 size={13} />
                            </button>
                          </ActionTooltip>
                        )}

                        <DropdownMenu>
                          <DropdownMenuTrigger asChild>
                            <button
                              type="button"
                              disabled={loadingPreviewId === att.id || downloadingAttId === att.id}
                              className="rounded-md p-1.5 text-[var(--text-tertiary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)] transition-colors disabled:opacity-50"
                              aria-label="More options"
                            >
                              {loadingPreviewId === att.id || downloadingAttId === att.id ? (
                                <Loader2 size={14} className="animate-spin text-[var(--text-secondary)]" />
                              ) : (
                                <MoreVertical size={14} />
                              )}
                            </button>
                          </DropdownMenuTrigger>
                          <DropdownMenuContent align="end" className="w-32">
                            <DropdownMenuItem
                              onSelect={() => handlePreviewAttachment(att)}
                              onMouseEnter={() => handlePrefetchAttachment(att)}
                              disabled={loadingPreviewId === att.id}
                              className="flex items-center gap-2 text-[12px] cursor-pointer"
                            >
                              {loadingPreviewId === att.id ? (
                                <Loader2 size={13} className="animate-spin text-[var(--text-secondary)]" />
                              ) : (
                                <Eye size={13} className="text-[var(--text-secondary)]" />
                              )}
                              <span>Preview</span>
                            </DropdownMenuItem>
                            <DropdownMenuItem
                              onSelect={() => handleDownloadAttachment(att)}
                              disabled={downloadingAttId === att.id}
                              className="flex items-center gap-2 text-[12px] cursor-pointer"
                            >
                              {downloadingAttId === att.id ? (
                                <Loader2 size={13} className="animate-spin text-[var(--text-secondary)]" />
                              ) : (
                                <Download size={13} className="text-[var(--text-secondary)]" />
                              )}
                              <span>Save</span>
                            </DropdownMenuItem>
                          </DropdownMenuContent>
                        </DropdownMenu>
                      </div>
                    </div>
                  ))}
                </div>
              </div>
            )}

            {/* Footer */}
            <div className="mt-auto flex gap-6 border-t border-[var(--border-subtle)] px-4 py-3 select-none">
              <span className="text-[12px] text-[var(--text-tertiary)]">
                {t('detail.created')}: {formatDate(data.createdAt)}
              </span>
              <span className="text-[12px] text-[var(--text-tertiary)]">
                {t('detail.updated')}: {formatDate(data.updatedAt)}
              </span>
            </div>
          </div>
        )}

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



      {/* Attachment Preview Modal */}
      <AttachmentPreviewModal
        open={!!previewAtt}
        onClose={handleClosePreview}
        attachment={previewAtt}
        data={previewData}
        onDownload={previewAtt ? () => handleDownloadAttachment(previewAtt) : undefined}
      />
    </div>
  );
}

function RecoveryCodesCard({
  codes,
  showRecovery,
  setShowRecovery,
  noBorder = false,
}: {
  codes: string;
  showRecovery: boolean;
  setShowRecovery: React.Dispatch<React.SetStateAction<boolean>>;
  noBorder?: boolean;
}) {
  const { t } = useTranslation();
  const { addToast } = useToast();
  const codeList = codes.split(/[\s,;\n]+/).filter(Boolean);

  return (
    <div className={noBorder ? 'select-none' : 'mt-2.5 border-t border-[var(--border-subtle)] pt-2.5 select-none'}>
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-1.5">
          <span className="text-[11px] font-semibold uppercase tracking-wide text-[var(--text-tertiary)]">
            {t('detail.recovery_codes')}
          </span>
          <span className="text-[10px] text-[var(--text-tertiary)]/70">
            ({codeList.length} keys)
          </span>
        </div>
        <button
          type="button"
          onClick={(e) => {
            e.stopPropagation();
            setShowRecovery((v) => !v);
          }}
          className="text-[11px] font-medium text-[var(--text-secondary)] hover:text-[var(--text-primary)] transition-colors cursor-pointer"
        >
          {showRecovery ? t('detail.hide_codes') : t('detail.show_codes')}
        </button>
      </div>

      {showRecovery && (
        <motion.div
          initial={{ opacity: 0, y: -4 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 0.1 }}
          className="mt-2.5 space-y-2"
        >
          <div className="grid grid-cols-2 gap-1.5 font-mono text-[12px] max-h-36 overflow-y-auto pr-1">
            {codeList.map((code, idx) => (
              <RecoveryCodeItem
                key={idx}
                code={code}
                index={idx}
                onCopy={() =>
                  addToast({
                    message: t('toast.recovery_code_copied', { index: idx + 1 }),
                    type: 'success',
                  })
                }
              />
            ))}
          </div>
          <div className="flex justify-end">
            <button
              type="button"
              onClick={async (e) => {
                e.stopPropagation();
                try {
                  if (isTauri()) {
                    const backend = await getBackend();
                    await backend.copyToClipboard(codes, true, 30);
                  } else {
                    await navigator.clipboard.writeText(codes);
                  }
                } catch {
                  if (!isTauri()) {
                    navigator.clipboard.writeText(codes).catch(() => {});
                  }
                }
                addToast({ message: t('detail.copied_all_recovery'), type: 'success' });
              }}
              className="rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] px-2.5 py-1 text-[11px] font-medium text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)] transition-colors cursor-pointer"
            >
              {t('detail.copy_all_codes')}
            </button>
          </div>
        </motion.div>
      )}
    </div>
  );
}

function RecoveryCodeItem({ code, index, onCopy }: { code: string; index: number; onCopy: () => void }) {
  const { t } = useTranslation();
  const [hovered, setHovered] = useState(false);
  const [copied, setCopied] = useState(false);

  const handleCopy = async () => {
    try {
      if (isTauri()) {
        const backend = await getBackend();
        await backend.copyToClipboard(code, true, 30);
      } else {
        await navigator.clipboard.writeText(code);
      }
    } catch {
      if (!isTauri()) {
        await navigator.clipboard.writeText(code).catch(() => {});
      }
    }
    setCopied(true);
    onCopy();
    setTimeout(() => setCopied(false), 1500);
  };

  return (
    <ActionTooltip content={copied ? t('detail.copied_code') : t('detail.click_copy_recovery_code')}>
      <button
        onClick={handleCopy}
        onMouseEnter={() => setHovered(true)}
        onMouseLeave={() => setHovered(false)}
        className="flex items-center justify-between rounded-[3px] border border-[var(--border-subtle)] bg-[var(--bg-elevated)] px-2.5 py-1.5 text-left transition-all hover:border-[var(--border-focus)] hover:bg-[var(--bg-hover)] active:scale-[0.98] group cursor-pointer w-full outline-none select-none"
      >
        <span className={`truncate font-medium font-mono tracking-wide ${hovered ? 'text-[var(--text-primary)] select-text' : 'text-[var(--text-primary)] select-none'}`}>
          {hovered ? code : '••••••••'}
        </span>
        <span className="text-[9px] text-[var(--text-tertiary)] shrink-0 select-none group-hover:text-[var(--text-secondary)] transition-colors ml-2">
          {copied ? t('common.copied') : `#${index + 1}`}
        </span>
      </button>
    </ActionTooltip>
  );
}

function TOTPField({
  secret,
  index,
  label = '2FA Code',
  children,
}: {
  secret: string;
  index: number;
  label?: string;
  children?: React.ReactNode;
}) {
  const { t } = useTranslation();
  const code = useTotp(secret);
  const isUrgent = code ? code.seconds_remaining <= 5 : false;

  // Format code with space in middle: "123 456"
  const formattedCode = code
    ? (code.code.length === 6 ? `${code.code.slice(0, 3)} ${code.code.slice(3)}` : code.code)
    : t('detail.generating');

  const progress = code ? code.seconds_remaining / code.period : 1;

  return (
    <motion.div
      initial={{ opacity: 0, y: 2 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ duration: 0.1, delay: index * 0.02 }}
      className="rounded-[3px] bg-[var(--bg-elevated)] px-3 py-2.5 transition-colors"
    >
      <div
        className={`flex items-center gap-3 -mx-3 px-3 py-2.5 transition-colors hover:bg-[var(--bg-hover)] ${
          children ? '-mt-2.5 rounded-t-[3px]' : '-my-2.5 rounded-[3px]'
        }`}
      >
        <span className="shrink-0 text-[var(--text-secondary)] select-none">
          <ShieldCheck size={15} />
        </span>
        <div className="min-w-0 flex-1">
          <div className="text-[11px] font-medium uppercase tracking-wide text-[var(--text-tertiary)] flex items-center gap-1.5 select-none">
            <span>{label}</span>
            {code && (
              <CountdownRing progress={progress} size={10} urgent={isUrgent} />
            )}
          </div>
          <div className="flex items-baseline gap-1.5">
            <span className={`font-mono text-[13px] font-semibold tracking-wider select-all ${
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
      </div>

      {children}
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
  const color = urgent ? '#ef4444' : 'var(--accent, #e8e8e8)';

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
  entryId?: string;
}> = ({ password, status, showTemporaryStats, entryId }) => {
  const [breachStatus, setBreachStatus] = useState<BreachStatus | null>(status || null);
  const { selectedEntry, updateBreachStatus, saveVault } = useEntries();
  const { audit } = useSecurityAudit();

  useEffect(() => {
    if (status) {
      setBreachStatus(status);
    }
  }, [status]);

  if (!password) return null;

  const reusedIssue = entryId
    ? audit?.issues.find((i) => i.entry_id === entryId && i.issue_type === 'ReusedPassword')
    : undefined;
  const weakIssue = entryId
    ? audit?.issues.find((i) => i.entry_id === entryId && i.issue_type === 'WeakPassword')
    : undefined;

  const isBreached = breachStatus?.type === 'Breached';
  const isReused = !!reusedIssue;
  const isWeak = !!weakIssue;
  const isSafe = (breachStatus?.type === 'Safe' || breachStatus?.type === 'Unknown') && !isReused && !isWeak;
  const shouldShowContainer = showTemporaryStats || !isSafe;

  if (!shouldShowContainer) return null;

  const borderAccentClass = isBreached
    ? 'border-l-2 border-l-red-500/80 bg-red-500/5'
    : isReused
      ? 'border-l-2 border-l-purple-500/80 bg-purple-500/5'
      : isWeak
        ? 'border-l-2 border-l-amber-500/80 bg-amber-500/5'
        : 'border-l-2 border-l-[var(--border-focus)]';

  return (
    <motion.div
      initial={{ opacity: 0 }}
      animate={{ opacity: 1 }}
      exit={{ opacity: 0 }}
      className={`flex flex-col gap-2 rounded-r-[3px] rounded-l-[2px] bg-[var(--bg-elevated)] px-3 py-2 mt-0.5 mb-2 transition-colors select-none ${borderAccentClass}`}
    >
      {showTemporaryStats && <PasswordStrength password={password} />}
      <BreachIndicator
        password={password}
        status={status}
        entryId={entryId}
        isReused={isReused}
        reusedServices={reusedIssue ? reusedIssue.description.split(': ')[1] : undefined}
        isWeak={isWeak}
        onStatusChange={async (newStatus) => {
          setBreachStatus(newStatus);
          if (selectedEntry) {
            try {
              await updateBreachStatus(selectedEntry.id, newStatus);
              await saveVault();
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

export default PasswordDetail;
