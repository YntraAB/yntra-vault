import React, { createContext, useContext, useState, useCallback, useMemo, useEffect, useRef } from 'react';
import type { PasswordEntry, Tag } from '@/types';
import type { EntryPreview, DecryptedEntry, BreachStatus } from '@/lib/backend';
import { getTranslation } from '@/i18n/translations';
import { useAuth } from '@/features/auth';
import { useSettings } from '@/features/settings';
import { useToast } from '@/contexts/ToastContext';
import { useUi, useSearch } from '@/contexts/UiContext';
import { getTransientWebdavPassword } from '@/lib/sessionSecrets';
import { sendDesktopNotification } from '@/lib/notifications';

// ─── Conversion helpers (Rust types ↔ frontend types) ───────────────────

export function entryPreviewToPasswordEntry(preview: EntryPreview, password = '••••••••'): PasswordEntry {
  return {
    id: preview.id,
    title: preview.title,
    username: preview.username,
    password,
    url: preview.url,
    email: preview.email,
    notes: '',
    tags: preview.tags,
    favorite: preview.favorite,
    pinned: preview.pinned,
    totpSecret: preview.has_totp ? 'has-totp' : undefined,
    customFields: [],
    createdAt: preview.updated_at,
    updatedAt: preview.updated_at,
    breachStatus: preview.breach_status,
    hasPasskey: preview.has_passkey || false,
    attachmentCount: preview.attachment_count || 0,
  };
}

export function isRecoveryField(name: string): boolean {
  if (!name) return false;
  const norm = name.trim().toLowerCase();
  return (
    norm === '2fa recovery codes' ||
    norm === '2fa recovery code' ||
    norm === '2fa backup codes' ||
    norm === 'recovery codes' ||
    norm === 'recovery code' ||
    norm === 'backup codes' ||
    norm.startsWith('2fa recovery cod')
  );
}

export function decryptedEntryToPasswordEntry(entry: DecryptedEntry): PasswordEntry {
  let recoveryCodes = '';
  const customFields = (entry.custom_fields || [])
    .filter((f) => {
      if (isRecoveryField(f.name)) {
        if (f.value && f.value.trim()) {
          recoveryCodes = recoveryCodes ? `${recoveryCodes}\n${f.value.trim()}` : f.value.trim();
        }
        return false;
      }
      return true;
    })
    .map((f) => ({
      id: f.id,
      name: f.name,
      type: f.field_type.toLowerCase() as any,
      value: f.value,
    }));

  const attachments = (entry.attachments || []).map((a) => ({
    id: a.id,
    name: a.name,
    size: a.size,
    mimeType: a.mime_type,
    createdAt: a.created_at,
  }));

  return {
    id: entry.id,
    title: entry.title,
    username: entry.username,
    password: entry.password,
    url: entry.url,
    email: entry.email,
    notes: entry.notes,
    tags: entry.tags,
    favorite: entry.favorite,
    pinned: entry.pinned,
    totpSecret: entry.totp_secret || undefined,
    recoveryCodes: recoveryCodes || undefined,
    customFields,
    createdAt: entry.created_at,
    updatedAt: entry.updated_at,
    breachStatus: entry.breach_status,
    hasPasskey: entry.has_passkey || false,
    passkeyPublicKey: entry.passkey_public_key || undefined,
    attachments,
    attachmentCount: attachments.length,
  };
}

// ─── Context Types ──────────────────────────────────────────────────────

export interface EntriesContextType {
  entries: PasswordEntry[];
  tags: Tag[];
  selectedEntry: PasswordEntry | null;
  selectedEntryIds: string[];
  isLoadingEntries: boolean;
  isLoadingDetail: boolean;
  setSelectedEntry: (entry: PasswordEntry | null) => void;
  setSelectedEntryIds: (ids: string[]) => void;
  toggleEntrySelection: (id: string, isMulti?: boolean, isRange?: boolean, orderedList?: PasswordEntry[]) => void;
  clearSelection: () => void;
  bulkUpdateEntries: (
    ids: string[],
    updates: Partial<PasswordEntry>,
    tagsToAdd?: string[],
    tagsToRemove?: string[],
    notesMode?: 'append' | 'overwrite'
  ) => Promise<void>;
  bulkDeleteEntries: (ids: string[]) => Promise<void>;
  updateEntry: (entry: PasswordEntry) => Promise<void>;
  updateBreachStatus: (id: string, breachStatus: BreachStatus) => Promise<void>;
  saveVault: () => Promise<void>;
  addEntry: (entry: PasswordEntry) => Promise<void>;
  deleteEntry: (id: string) => Promise<void>;
  deleteAttachment: (entryId: string, attachmentId: string) => Promise<void>;
  toggleFavorite: (id: string) => Promise<void>;
  togglePin: (id: string) => Promise<void>;
  selectEntryById: (id: string | null) => Promise<void>;
  addTag: (tag: Tag) => Promise<void>;
  updateTag: (id: string, updates: Partial<Tag>) => Promise<void>;
  removeTag: (id: string) => Promise<void>;
  reorderTags: (newTags: Tag[]) => Promise<void>;
  refreshEntries: () => Promise<void>;
  refreshTags: () => Promise<void>;
  isP2pListening: boolean;
  toggleP2pListener: (enable?: boolean) => void;
  localLanIp: string | null;
}

const EntriesContext = createContext<EntriesContextType | undefined>(undefined);

export function EntriesProvider({ children }: { children: React.ReactNode }) {
  const { backend, currentVault, isLocked } = useAuth();
  const { settings } = useSettings();
  const { addToast } = useToast();

  const [isLoadingEntries, setIsLoadingEntries] = useState(false);
  const [isP2pListening, setIsP2pListening] = useState(false);
  const [localLanIp, setLocalLanIp] = useState<string | null>(null);
  const [listenerManualOverride, setListenerManualOverride] = useState<boolean | null>(null);
  const [isLoadingDetail, setIsLoadingDetail] = useState(false);
  const [entries, setEntries] = useState<PasswordEntry[]>([]);
  const [rawTags, setRawTags] = useState<Tag[]>([]);
  const [selectedEntry, setSelectedEntry] = useState<PasswordEntry | null>(null);
  const [selectedEntryIds, setSelectedEntryIds] = useState<string[]>([]);

  const selectedEntryRef = useRef<PasswordEntry | null>(selectedEntry);
  useEffect(() => {
    selectedEntryRef.current = selectedEntry;
  }, [selectedEntry]);

  const rawTagsRef = useRef<Tag[]>(rawTags);
  useEffect(() => {
    rawTagsRef.current = rawTags;
  }, [rawTags]);

  const isLockedRef = useRef(isLocked);
  useEffect(() => {
    isLockedRef.current = isLocked;
  }, [isLocked]);

  const selectionSeqRef = useRef(0);
  const refreshSeqRef = useRef(0);
  const tagRefreshSeqRef = useRef(0);

  // Purge sensitive state from JS memory when vault is locked (V8 Heap Hygiene)
  useEffect(() => {
    if (isLocked) {
      selectionSeqRef.current++;
      refreshSeqRef.current++;
      tagRefreshSeqRef.current++;
      if (typeof document !== 'undefined' && document.activeElement) {
        try {
          (document.activeElement as HTMLElement).blur();
        } catch {}
      }
      setEntries([]);
      setRawTags([]);
      setSelectedEntry(null);
      setSelectedEntryIds([]);

      if (backend) {
        backend.lockVault().catch((err) => {
          console.error('Failed to lock backend vault on lock state change:', err);
        });
      }
    }
  }, [isLocked, backend]);

  // Load entries from backend when vault is opened
  const refreshEntries = useCallback(async () => {
    if (!backend || !currentVault || isLockedRef.current) return;
    const request = ++refreshSeqRef.current;
    try {
      const previews = await backend.listEntries();
      if (isLockedRef.current || request !== refreshSeqRef.current) return;
      const entriesList = previews.map((p) => entryPreviewToPasswordEntry(p));

      setEntries(entriesList);

      // Revalidate active selected entry if it changed remotely or was deleted
      const currentSelected = selectedEntryRef.current;
      if (currentSelected && !isLockedRef.current) {
        const matchingPreview = entriesList.find((p) => p.id === currentSelected.id);
        if (!matchingPreview) {
          setSelectedEntry(null);
        } else {
          const selection = selectionSeqRef.current;
          try {
            const full = await backend.getEntry(currentSelected.id);
            if (!isLockedRef.current && request === refreshSeqRef.current && selection === selectionSeqRef.current && selectedEntryRef.current?.id === currentSelected.id) {
              setSelectedEntry(decryptedEntryToPasswordEntry(full));
            }
          } catch (err) {
            console.error('Failed to reload remotely updated entry:', err);
          }
        }
      }
    } catch (e) {
      console.error('Failed to load entries:', e);
    }
  }, [backend, currentVault]);

  useEffect(() => () => {
    refreshSeqRef.current++;
    tagRefreshSeqRef.current++;
    selectionSeqRef.current++;
  }, [currentVault?.id, currentVault?.path]);

  // Load tags from backend when vault is opened
  const refreshTags = useCallback(async () => {
    if (!backend || !currentVault || isLockedRef.current) return;
    const request = ++tagRefreshSeqRef.current;
    try {
      const dbTags = await backend.getTags();
      if (isLockedRef.current || request !== tagRefreshSeqRef.current) return;
      setRawTags(
        dbTags.map((t) => ({
          id: t.id,
          name: t.name,
          color: t.color || '#5b8def',
          icon: t.icon || 'tag',
          count: 0,
        }))
      );
    } catch (e) {
      console.error('Failed to load tags:', e);
    }
  }, [backend, currentVault]);

  // Auto-refresh when vault unlocks or changes
  useEffect(() => {
    if (backend) {
      if (currentVault && !isLocked) {
        const loadAll = async () => {
          setIsLoadingEntries(true);
          const startTime = Date.now();
          try {
            await Promise.all([refreshEntries(), refreshTags()]);
            const elapsed = Date.now() - startTime;
            if (!settings.disableSkeletonDelays && elapsed < 200) {
              await new Promise((resolve) => setTimeout(resolve, 200 - elapsed));
            }
          } finally {
            setIsLoadingEntries(false);
          }
        };
        loadAll();
      } else {
        setEntries([]);
        setRawTags([]);
        setSelectedEntry(null);
      }
    }
  }, [backend, currentVault, isLocked, refreshEntries, refreshTags, settings.disableSkeletonDelays]);

  // Fetch full entry details when selecting (Tauri mode)
  const selectEntryById = useCallback(
    async (id: string | null) => {
      const seq = ++selectionSeqRef.current;

      if (!id || isLockedRef.current) {
        setSelectedEntry(null);
        return;
      }

      const isSameEntry = selectedEntryRef.current?.id === id;
      if (!isSameEntry) {
        setIsLoadingDetail(true);
      }

      const startTime = Date.now();
      if (backend) {
        try {
          const full = await backend.getEntry(id);
          if (seq !== selectionSeqRef.current || isLockedRef.current) {
            return;
          }

          const entry = decryptedEntryToPasswordEntry(full);
          if (!isSameEntry && !settings.disableSkeletonDelays) {
            const elapsed = Date.now() - startTime;
            if (elapsed < 150) {
              await new Promise((resolve) => setTimeout(resolve, 150 - elapsed));
            }
            if (seq !== selectionSeqRef.current || isLockedRef.current) {
              return;
            }
          }
          setSelectedEntry(entry);
        } catch (e) {
          if (seq === selectionSeqRef.current && !isLockedRef.current) {
            setSelectedEntry(entries.find((e) => e.id === id) || null);
          }
        } finally {
          if (!isSameEntry && seq === selectionSeqRef.current) {
            setIsLoadingDetail(false);
          }
        }
      } else {
        // Mock mode
        if (!isSameEntry && !settings.disableSkeletonDelays) {
          await new Promise((resolve) => setTimeout(resolve, 200));
        }
        if (seq !== selectionSeqRef.current || isLockedRef.current) {
          return;
        }
        const entry = entries.find((e) => e.id === id) || null;
        setSelectedEntry(entry);
        if (!isSameEntry && seq === selectionSeqRef.current) {
          setIsLoadingDetail(false);
        }
      }
    },
    [backend, entries, settings.disableSkeletonDelays]
  );

  // Dynamic tag counts
  const tags = useMemo(() => {
    const countMap: Record<string, number> = {};
    for (const entry of entries) {
      for (const tagName of entry.tags) {
        countMap[tagName] = (countMap[tagName] || 0) + 1;
      }
    }
    return rawTags.map((tag) => ({ ...tag, count: countMap[tag.name] || 0 }));
  }, [rawTags, entries]);

  // Debounced auto-sync trigger for WebDAV cloud sync on changes
  const autoSyncTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const triggerAutoSync = useCallback(() => {
    if (!settings.webdavEnabled || !settings.webdavAutoSync || !settings.webdavUrl || !settings.webdavUser || !backend) {
      return;
    }
    if (autoSyncTimerRef.current) {
      clearTimeout(autoSyncTimerRef.current);
    }
    autoSyncTimerRef.current = setTimeout(async () => {
      try {
        const pass = getTransientWebdavPassword();
        const stats = await backend.webdavSync(settings.webdavUrl!, settings.webdavUser!, pass);
        if (stats && (stats.entries_added > 0 || stats.entries_updated > 0 || stats.tags_merged > 0 || stats.trash_merged > 0)) {
          await Promise.all([refreshEntries(), refreshTags()]);
        }
      } catch (err) {
        console.warn('Auto-sync on save failed:', err);
      }
    }, 2000);
  }, [settings.webdavEnabled, settings.webdavAutoSync, settings.webdavUrl, settings.webdavUser, backend, refreshEntries, refreshTags]);

  const toggleP2pListener = useCallback((enable?: boolean) => {
    setListenerManualOverride((prev) => {
      const current = prev !== null ? prev : Boolean(settings.p2pAutoListen);
      return enable !== undefined ? enable : !current;
    });
  }, [settings.p2pAutoListen]);

  // Fetch local LAN IP
  useEffect(() => {
    if (backend) {
      backend.getLocalIp().then((ip) => setLocalLanIp(ip || null)).catch(() => {});
    }
  }, [backend]);

  // Reset override when locked
  useEffect(() => {
    if (isLocked) {
      setListenerManualOverride(null);
      setIsP2pListening(false);
    }
  }, [isLocked]);

  const shouldListen = (listenerManualOverride !== null ? listenerManualOverride : Boolean(settings.p2pAutoListen)) && !isLocked && Boolean(backend) && Boolean(currentVault);

  const listenerTail = useRef<Promise<void>>(Promise.resolve());
  const listenerView = useRef({ refreshEntries, refreshTags, addToast, language: settings.language });
  listenerView.current = { refreshEntries, refreshTags, addToast, language: settings.language };

  // P2P Auto-Listener / Background loop
  useEffect(() => {
    if (!shouldListen || !backend || !currentVault || isLocked) {
      setIsP2pListening(false);
      return;
    }

    let isCancelled = false;
    const previous = listenerTail.current;

    const runListenerLoop = async () => {
      await previous.catch(() => {});
      if (isCancelled || isLockedRef.current) return;
      const unlisten = await backend.onP2pListenerReady(() => {
        if (!isCancelled && !isLockedRef.current) setIsP2pListening(true);
      });
      try {
      while (!isCancelled && !isLockedRef.current) {
        try {
          setIsP2pListening(false);
          const addr = settings.p2pAddr || '0.0.0.0:5322';
          const stats = await backend.runP2pSyncListener(addr, currentVault.path);
          if (!isCancelled && !isLockedRef.current) {
            const { refreshEntries, refreshTags, addToast, language } = listenerView.current;
            await Promise.all([refreshEntries(), refreshTags()]);
            const lang = language || 'en';
            const count = (stats?.entries_added || 0) + (stats?.entries_updated || 0);
            const msg = count > 0
              ? getTranslation(lang, 'sync.toast_peer_synced_changes', { count: String(count) })
              : getTranslation(lang, 'sync.toast_peer_synced_clean');
            addToast({ message: msg, type: 'success' });
            sendDesktopNotification(getTranslation(lang, 'sync.desktop_title'), msg);
          }
        } catch (error) {
          setIsP2pListening(false);
          if (!isCancelled && String(error).includes('Failed to bind TCP listener')) {
            listenerView.current.addToast({ message: String(error), type: 'error' });
            break;
          }
          // Timeout or connection closed; wait 1.5s before listening again
          if (isCancelled || isLockedRef.current) break;
          await new Promise((r) => setTimeout(r, 1500));
        }
      }
      } finally {
        unlisten();
        if (!isCancelled) setIsP2pListening(false);
      }
    };

    const running = runListenerLoop().catch((error) => {
      if (!isCancelled) { setIsP2pListening(false); listenerView.current.addToast({ message: String(error), type: 'error' }); }
    });
    listenerTail.current = running;
    return () => {
      isCancelled = true;
      const stopping = backend.cancelP2pSyncListener().catch(() => {});
      listenerTail.current = Promise.all([running, stopping]).then(() => {});
      setIsP2pListening(false);
    };
  }, [shouldListen, settings.p2pAddr, backend, currentVault?.path, isLocked]);

  // P2P Auto-Sync on Wi-Fi (Periodically scans for beacons and syncs)
  useEffect(() => {
    if (!settings.p2pAutoSyncWifi || !backend || !currentVault || isLocked) {
      return;
    }

    let isCancelled = false;
    const intervalMs = (settings.p2pAutoSyncIntervalMinutes || 5) * 60 * 1000;

    let syncing = false;
    const runDiscoverySync = async () => {
      if (syncing || isCancelled) return;
      syncing = true;
      try {
        const peer = await backend.scanP2pDiscovery(2500);
        if (peer && !isCancelled && !isLockedRef.current) {
          const stats = await backend.runP2pSyncClient(peer, currentVault.path);
          await Promise.all([refreshEntries(), refreshTags()]);
          const lang = settings.language || 'en';
          const count = (stats?.entries_added || 0) + (stats?.entries_updated || 0);
          const msg = count > 0
            ? getTranslation(lang, 'sync.toast_auto_sync_changes', { count: String(count) })
            : getTranslation(lang, 'sync.toast_auto_sync_clean');
          addToast({ message: msg, type: 'success' });
          sendDesktopNotification(getTranslation(lang, 'sync.desktop_title'), msg);
        }
      } catch (err: any) {
        const errStr = String(err?.message || err || '');
        if (errStr.includes('disconnected by the host') || errStr.includes('re-pair')) {
          addToast({
            message: getTranslation(settings.language || 'en', 'pairing.device_revoked_notice'),
            type: 'error',
          });
        }
      } finally { syncing = false; }
    };

    // Initial check shortly after unlock
    const initTimer = setTimeout(runDiscoverySync, 3000);
    const interval = setInterval(runDiscoverySync, intervalMs);

    return () => {
      isCancelled = true;
      clearTimeout(initTimer);
      clearInterval(interval);
    };
  }, [settings.p2pAutoSyncWifi, settings.p2pAutoSyncIntervalMinutes, settings.language, backend, currentVault, isLocked, refreshEntries, refreshTags, addToast]);

  // CRUD Operations
  // CRUD Operations with SOTA Optimistic Local-First State & Background Persistence
  const updateEntry = useCallback(
    async (entry: PasswordEntry) => {
      const now = new Date().toISOString();
      const cleanedCustomFields = entry.customFields.filter(f => !isRecoveryField(f.name));
      let recoveryCodes = entry.recoveryCodes;
      if (!recoveryCodes) {
        const found = entry.customFields.filter(f => isRecoveryField(f.name));
        if (found.length) recoveryCodes = found.map(f => f.value).join('\n');
      }
      const updatedEntry: PasswordEntry = {
        ...entry,
        recoveryCodes,
        customFields: cleanedCustomFields,
        updatedAt: now,
      };

      // 1. Snapshot previous state for rollback
      const prevEntries = entries;
      const prevSelected = selectedEntry;

      // 2. Optimistic update immediately (0ms visual latency)
      setEntries((prev) =>
        prev.map((e) => (e.id === entry.id ? updatedEntry : e))
      );
      setSelectedEntry((prev) => (prev?.id === entry.id ? updatedEntry : prev));

      // 3. Persist to backend asynchronously
      if (backend) {
        try {
          const customFieldsToSend = [
            ...cleanedCustomFields.map((f) => ({
              id: f.id,
              name: f.name,
              field_type: (f.type.charAt(0).toUpperCase() + f.type.slice(1)) as any,
              value: f.value,
              sensitive: f.type === 'password',
            })),
          ];
          if (recoveryCodes && recoveryCodes.trim()) {
            customFieldsToSend.push({
              id: crypto.randomUUID(),
              name: '2FA Recovery Codes',
              field_type: 'Password' as any,
              value: recoveryCodes.trim(),
              sensitive: true,
            });
          }

          const updatePayload: any = {
            title: entry.title,
            username: entry.username,
            password: entry.password,
            url: entry.url,
            email: entry.email,
            notes: entry.notes,
            tags: entry.tags,
            favorite: entry.favorite,
            pinned: entry.pinned,
            custom_fields: customFieldsToSend,
          };

          if (entry.totpSecret !== undefined && entry.totpSecret !== 'has-totp') {
            updatePayload.totp_secret = entry.totpSecret;
          }

          if (entry.passkeyAction) {
            updatePayload.passkey_action = entry.passkeyAction;
          }

          if (entry.newAttachments && entry.newAttachments.length > 0) {
            updatePayload.new_attachments = entry.newAttachments;
          }

          if (entry.deleteAttachmentIds && entry.deleteAttachmentIds.length > 0) {
            updatePayload.delete_attachment_ids = entry.deleteAttachmentIds;
          }

          await backend.updateEntry(entry.id, updatePayload);

          // If attachments or passkeys were staged, fetch refreshed entry metadata without reloading whole DB
          if (entry.newAttachments?.length || entry.deleteAttachmentIds?.length || entry.passkeyAction) {
            const updatedDetail = await backend.getEntry(entry.id).catch(() => null);
            if (updatedDetail) {
              const fullEntry = decryptedEntryToPasswordEntry(updatedDetail);
              setSelectedEntry((prev) => (prev?.id === entry.id ? fullEntry : prev));
              setEntries((prev) =>
                prev.map((e) => (e.id === entry.id ? { ...fullEntry, password: '••••••••' } : e))
              );
            }
          }
          triggerAutoSync();
        } catch (e) {
          // 4. Rollback on failure
          setEntries(prevEntries);
          setSelectedEntry(prevSelected);
          addToast({
            message: getTranslation(settings.language, 'toast.update_failed', { err: String(e) }),
            type: 'error',
          });
        }
      }
    },
    [backend, entries, selectedEntry, settings.language, addToast, triggerAutoSync]
  );

  const updateBreachStatus = useCallback(
    async (id: string, newStatus: BreachStatus) => {
      setEntries((prev) => prev.map((e) => (e.id === id ? { ...e, breachStatus: newStatus } : e)));
      setSelectedEntry((prev) => (prev?.id === id ? { ...prev, breachStatus: newStatus } : prev));
      if (backend) {
        await backend.updateEntryBreachStatus(id, newStatus);
      }
    },
    [backend]
  );

  const saveVault = useCallback(async () => {
    if (backend) {
      await backend.saveVault();
      triggerAutoSync();
    }
  }, [backend, triggerAutoSync]);

  const addEntry = useCallback(
    async (entry: PasswordEntry) => {
      const now = new Date().toISOString();
      const cleanedCustomFields = entry.customFields.filter(f => !isRecoveryField(f.name));
      let recoveryCodes = entry.recoveryCodes;
      if (!recoveryCodes) {
        const found = entry.customFields.filter(f => isRecoveryField(f.name));
        if (found.length) recoveryCodes = found.map(f => f.value).join('\n');
      }
      const customFieldsToSend = cleanedCustomFields.map((f) => ({
        id: f.id,
        name: f.name,
        field_type: (f.type.charAt(0).toUpperCase() + f.type.slice(1)) as any,
        value: f.value,
        sensitive: f.type === 'password',
      }));
      if (recoveryCodes && recoveryCodes.trim()) {
        customFieldsToSend.push({
          id: crypto.randomUUID(),
          name: '2FA Recovery Codes',
          field_type: 'Password' as any,
          value: recoveryCodes.trim(),
          sensitive: true,
        });
      }

      if (backend) {
        try {
          const newId = await backend.addEntry({
            title: entry.title,
            username: entry.username,
            password: entry.password,
            url: entry.url,
            email: entry.email,
            notes: entry.notes,
            tags: entry.tags,
            totp_secret: entry.totpSecret || null,
            custom_fields: customFieldsToSend,
            entry_type: null,
            generate_passkey: entry.generatePasskey,
            attachments: entry.newAttachments,
          });

          // In-memory instant insertion without full DB reload across IPC
          const createdEntry: PasswordEntry = {
            ...entry,
            id: newId,
            createdAt: entry.createdAt || now,
            updatedAt: entry.updatedAt || now,
            attachmentCount: (entry.newAttachments?.length || 0) + (entry.attachments?.length || 0),
          };

          setEntries((prev) => [createdEntry, ...prev.filter((e) => e.id !== newId)]);
          setSelectedEntry(createdEntry);
          triggerAutoSync();
        } catch (e) {
          addToast({
            message: getTranslation(settings.language, 'toast.add_failed', { err: String(e) }),
            type: 'error',
          });
          throw e;
        }
      } else {
        const localEntry: PasswordEntry = {
          ...entry,
          createdAt: entry.createdAt || now,
          updatedAt: entry.updatedAt || now,
        };
        setEntries((prev) => [localEntry, ...prev]);
        setSelectedEntry(localEntry);
      }
    },
    [backend, settings.language, addToast, triggerAutoSync]
  );

  const deleteEntry = useCallback(
    async (id: string) => {
      // 1. Snapshot previous state for rollback
      const prevEntries = entries;
      const prevSelected = selectedEntry;

      // 2. Optimistic instant removal from list (0ms visual latency)
      setEntries((prev) => prev.filter((e) => e.id !== id));
      if (selectedEntry?.id === id) {
        setSelectedEntry(null);
      }
      addToast({ message: getTranslation(settings.language, 'toast.moved_to_trash'), type: 'info' });

      // 3. Persist deletion in background
      if (backend) {
        try {
          await backend.deleteEntry(id);
          triggerAutoSync();
        } catch (e) {
          // 4. Rollback on failure
          setEntries(prevEntries);
          setSelectedEntry(prevSelected);
          addToast({
            message: getTranslation(settings.language, 'toast.delete_failed', { err: String(e) }),
            type: 'error',
          });
        }
      }
    },
    [backend, entries, selectedEntry, settings.language, addToast, triggerAutoSync]
  );

  const deleteAttachment = useCallback(
    async (entryId: string, attachmentId: string) => {
      const prevEntries = entries;
      const prevSelected = selectedEntry;

      // Optimistic update
      setSelectedEntry((prev) => {
        if (prev?.id !== entryId) return prev;
        const updatedAtts = (prev.attachments || []).filter((a) => a.id !== attachmentId);
        return {
          ...prev,
          attachments: updatedAtts,
          attachmentCount: updatedAtts.length,
        };
      });
      setEntries((prev) =>
        prev.map((e) => {
          if (e.id !== entryId) return e;
          return {
            ...e,
            attachmentCount: Math.max(0, (e.attachmentCount || 1) - 1),
          };
        })
      );

      if (backend) {
        try {
          await backend.deleteAttachment(entryId, attachmentId);
        } catch (e) {
          setSelectedEntry(prevSelected);
          setEntries(prevEntries);
          addToast({
            message: getTranslation(settings.language, 'toast.delete_failed', { err: String(e) }),
            type: 'error',
          });
        }
      }
    },
    [backend, entries, selectedEntry, settings.language, addToast]
  );

  const toggleFavorite = useCallback(
    async (id: string) => {
      // 1. Snapshot previous state for rollback
      const prevEntries = entries;
      const prevSelected = selectedEntry;

      // 2. Optimistic instant update in memory (0ms visual latency)
      setEntries((prev) =>
        prev.map((e) => (e.id === id ? { ...e, favorite: !e.favorite } : e))
      );
      setSelectedEntry((prev) =>
        prev?.id === id ? { ...prev, favorite: !prev.favorite } : prev
      );

      // 3. Persist to backend in background
      if (backend) {
        try {
          await backend.toggleFavorite(id);
          triggerAutoSync();
        } catch (e) {
          // 4. Rollback on failure
          setEntries(prevEntries);
          setSelectedEntry(prevSelected);
          console.error('Toggle favorite failed:', e);
          addToast({
            message: getTranslation(settings.language, 'toast.action_failed', { err: String(e) }),
            type: 'error',
          });
        }
      }
    },
    [backend, entries, selectedEntry, settings.language, addToast, triggerAutoSync]
  );

  const togglePin = useCallback(
    async (id: string) => {
      // 1. Snapshot previous state for rollback
      const prevEntries = entries;
      const prevSelected = selectedEntry;

      // 2. Optimistic instant update in memory (0ms visual latency)
      setEntries((prev) =>
        prev.map((e) => (e.id === id ? { ...e, pinned: !e.pinned } : e))
      );
      setSelectedEntry((prev) =>
        prev?.id === id ? { ...prev, pinned: !prev.pinned } : prev
      );

      // 3. Persist to backend in background
      if (backend) {
        try {
          await backend.togglePin(id);
          triggerAutoSync();
        } catch (e) {
          // 4. Rollback on failure
          setEntries(prevEntries);
          setSelectedEntry(prevSelected);
          console.error('Toggle pin failed:', e);
          addToast({
            message: getTranslation(settings.language, 'toast.action_failed', { err: String(e) }),
            type: 'error',
          });
        }
      }
    },
    [backend, entries, selectedEntry, settings.language, addToast, triggerAutoSync]
  );

  // Multi-selection & Bulk operations
  const toggleEntrySelection = useCallback(
    (id: string, isMulti = false, isRange = false, orderedList?: PasswordEntry[]) => {
      if (isRange && selectedEntryIds.length > 0) {
        const list = orderedList || entries;
        const lastSelectedId = selectedEntryIds[selectedEntryIds.length - 1];
        const lastIndex = list.findIndex((e) => e.id === lastSelectedId);
        const currentIndex = list.findIndex((e) => e.id === id);
        if (lastIndex !== -1 && currentIndex !== -1) {
          const start = Math.min(lastIndex, currentIndex);
          const end = Math.max(lastIndex, currentIndex);
          const rangeIds = list.slice(start, end + 1).map((e) => e.id);
          const merged = Array.from(new Set([...selectedEntryIds, ...rangeIds]));
          setSelectedEntryIds(merged);
          return;
        }
      }

      if (isMulti) {
        setSelectedEntryIds((prev) =>
          prev.includes(id) ? prev.filter((i) => i !== id) : [...prev, id]
        );
      } else {
        setSelectedEntryIds([id]);
      }
    },
    [entries, selectedEntryIds]
  );

  const clearSelection = useCallback(() => {
    setSelectedEntryIds([]);
  }, []);

  const bulkUpdateEntries = useCallback(
    async (
      ids: string[],
      updates: Partial<PasswordEntry>,
      tagsToAdd: string[] = [],
      tagsToRemove: string[] = [],
      notesMode: 'append' | 'overwrite' = 'append'
    ) => {
      if (ids.length === 0) return;

      const prevEntries = entries;
      const prevSelected = selectedEntry;

      // 1. Optimistically patch entries in memory immediately (0ms visual latency)
      setEntries((prev) =>
        prev.map((e) => {
          if (!ids.includes(e.id)) return e;

          let updatedTags = [...e.tags];
          tagsToAdd.forEach((t) => {
            if (!updatedTags.includes(t)) updatedTags.push(t);
          });
          tagsToRemove.forEach((t) => {
            updatedTags = updatedTags.filter((tag) => tag !== t);
          });

          let updatedNotes = e.notes;
          if (updates.notes !== undefined) {
            updatedNotes =
              notesMode === 'append' && e.notes ? `${e.notes}\n${updates.notes}` : updates.notes;
          }

          return {
            ...e,
            ...updates,
            tags: updatedTags,
            notes: updatedNotes,
            updatedAt: new Date().toISOString(),
          };
        })
      );

      if (selectedEntry && ids.includes(selectedEntry.id)) {
        let updatedTags = [...selectedEntry.tags];
        tagsToAdd.forEach((t) => {
          if (!updatedTags.includes(t)) updatedTags.push(t);
        });
        tagsToRemove.forEach((t) => {
          updatedTags = updatedTags.filter((tag) => tag !== t);
        });
        let updatedNotes = selectedEntry.notes;
        if (updates.notes !== undefined) {
          updatedNotes =
            notesMode === 'append' && selectedEntry.notes
              ? `${selectedEntry.notes}\n${updates.notes}`
              : updates.notes;
        }
        setSelectedEntry({
          ...selectedEntry,
          ...updates,
          tags: updatedTags,
          notes: updatedNotes,
          updatedAt: new Date().toISOString(),
        });
      }

      addToast({
        message: getTranslation(settings.language, 'toast.bulk_updated', { count: ids.length }),
        type: 'info',
      });

      // 2. Persist to backend asynchronously in background
      if (backend) {
        try {
          for (const id of ids) {
            const entry = prevEntries.find((e) => e.id === id);
            if (!entry) continue;

            const patchData: any = {};
            if (updates.title !== undefined) patchData.title = updates.title;
            if (updates.username !== undefined) patchData.username = updates.username;
            if (updates.url !== undefined) patchData.url = updates.url;
            if (updates.favorite !== undefined) patchData.favorite = updates.favorite;
            if (updates.pinned !== undefined) patchData.pinned = updates.pinned;

            if (tagsToAdd.length > 0 || tagsToRemove.length > 0) {
              let updatedTags = [...entry.tags];
              tagsToAdd.forEach((t) => {
                if (!updatedTags.includes(t)) updatedTags.push(t);
              });
              tagsToRemove.forEach((t) => {
                updatedTags = updatedTags.filter((tag) => tag !== t);
              });
              patchData.tags = updatedTags;
            }

            if (updates.notes !== undefined) {
              if (notesMode === 'append') {
                const fullEntry = await backend.getEntry(id).catch(() => null);
                const currentNotes = fullEntry?.notes || '';
                patchData.notes = currentNotes ? `${currentNotes}\n${updates.notes}` : updates.notes;
              } else {
                patchData.notes = updates.notes;
              }
            }

            await backend.updateEntry(id, patchData);
          }
          triggerAutoSync();
        } catch (e) {
          // 3. Rollback on failure
          setEntries(prevEntries);
          setSelectedEntry(prevSelected);
          addToast({
            message: getTranslation(settings.language, 'toast.action_failed', { err: String(e) }),
            type: 'error',
          });
        }
      }
    },
    [backend, entries, selectedEntry, settings.language, addToast, triggerAutoSync]
  );

  const bulkDeleteEntries = useCallback(
    async (ids: string[]) => {
      if (ids.length === 0) return;

      const prevEntries = entries;
      const prevSelected = selectedEntry;
      const prevSelectedIds = selectedEntryIds;

      // Optimistic instant removal from UI
      setEntries((prev) => prev.filter((e) => !ids.includes(e.id)));
      if (selectedEntry && ids.includes(selectedEntry.id)) {
        setSelectedEntry(null);
      }
      setSelectedEntryIds([]);
      addToast({ message: `Deleted ${ids.length} entries`, type: 'info' });

      if (backend) {
        try {
          for (const id of ids) {
            await backend.deleteEntry(id);
          }
          triggerAutoSync();
        } catch (e) {
          // Rollback on failure
          setEntries(prevEntries);
          setSelectedEntry(prevSelected);
          setSelectedEntryIds(prevSelectedIds);
          addToast({ message: `Bulk delete failed: ${e}`, type: 'error' });
        }
      }
    },
    [backend, entries, selectedEntry, selectedEntryIds, addToast, triggerAutoSync]
  );

  // Tag CRUD with SOTA Optimistic Updates & Rollback
  const addTag = useCallback(
    async (tag: Tag) => {
      const prevTags = rawTags;
      setRawTags((prev) => [...prev, tag]);

      if (backend) {
        try {
          const realId = await backend.addTag(tag.name, tag.color, tag.icon);
          setRawTags((prev) => prev.map((t) => (t.id === tag.id ? { ...t, id: realId } : t)));
          triggerAutoSync();
        } catch (e) {
          setRawTags(prevTags);
          addToast({ message: `Failed to create tag: ${e}`, type: 'error' });
        }
      }
    },
    [backend, rawTags, addToast, triggerAutoSync]
  );

  const updateTag = useCallback(
    async (id: string, updates: Partial<Tag>) => {
      const oldTag = rawTags.find((t) => t.id === id);
      if (!oldTag) return;

      const prevTags = rawTags;
      const prevEntries = entries;
      const nextTag = { ...oldTag, ...updates };

      setRawTags((prev) => prev.map((t) => (t.id === id ? nextTag : t)));

      // If name changed, optimistically update entry tags
      if (updates.name && updates.name !== oldTag.name) {
        const oldName = oldTag.name;
        const newName = updates.name;
        setEntries((prev) =>
          prev.map((e) =>
            e.tags.includes(oldName)
              ? { ...e, tags: e.tags.map((t) => (t === oldName ? newName : t)) }
              : e
          )
        );
      }

      if (backend) {
        try {
          await backend.updateTag(id, nextTag.name, nextTag.color, nextTag.icon);
          triggerAutoSync();
        } catch (e) {
          setRawTags(prevTags);
          setEntries(prevEntries);
          addToast({ message: `Failed to update tag in database: ${e}`, type: 'error' });
        }
      }
    },
    [backend, rawTags, entries, addToast, triggerAutoSync]
  );

  const removeTag = useCallback(
    async (id: string) => {
      const tag = rawTags.find((t) => t.id === id);
      if (!tag) return;

      const prevTags = rawTags;
      const prevEntries = entries;

      setRawTags((prev) => prev.filter((t) => t.id !== id));
      setEntries((entries) =>
        entries.map((e) => ({
          ...e,
          tags: e.tags.filter((t) => t !== tag.name),
        }))
      );

      if (backend) {
        try {
          await backend.deleteTag(id);
          triggerAutoSync();
        } catch (e) {
          setRawTags(prevTags);
          setEntries(prevEntries);
          addToast({ message: `Failed to delete tag: ${e}`, type: 'error' });
        }
      }
    },
    [backend, rawTags, entries, addToast, triggerAutoSync]
  );

  const reorderTags = useCallback(
    async (newTags: Tag[]) => {
      const prevTags = rawTagsRef.current;
      setRawTags(newTags);

      if (backend) {
        try {
          await backend.reorderTags(newTags.map((t) => t.id));
          triggerAutoSync();
        } catch (e) {
          setRawTags(prevTags);
          addToast({ message: `Failed to save tag order: ${e}`, type: 'error' });
        }
      }
    },
    [backend, addToast, triggerAutoSync]
  );

  // Silent background vault-wide breach check on vault unlock
  const entriesRef = useRef(entries);
  useEffect(() => {
    entriesRef.current = entries;
  }, [entries]);

  useEffect(() => {
    if (!backend || isLocked || !currentVault || entries.length === 0 || !settings.autoBreachCheck) {
      return;
    }

    let active = true;
    let timeoutId: any = null;
    let hasChangesToSave = false;

    const checkNext = async () => {
      if (!active) return;

      const target = entriesRef.current.find((e) => !e.breachStatus || e.breachStatus.type === 'Unknown');

      if (!target) {
        if (hasChangesToSave && backend) {
          hasChangesToSave = false;
          try {
            await backend.saveVault();
          } catch (err) {
            console.error('Failed to save vault after breach checks:', err);
          }
        }
        return;
      }

      try {
        const decryptedRaw = await backend.getEntry(target.id);
        const decrypted = decryptedEntryToPasswordEntry(decryptedRaw);
        if (!active) return;

        const passwordValue = decrypted.password;
        if (passwordValue && passwordValue.trim() !== '') {
          const result = await backend.checkPasswordBreach(passwordValue);
          if (!active) return;

          const wasSafe = target.breachStatus?.type === 'Safe';
          const newStatus = result.is_breached
            ? { type: 'Breached' as const, breach_count: result.breach_count, checked_at: result.checked_at }
            : { type: 'Safe' as const, checked_at: result.checked_at };

          if (wasSafe && newStatus.type === 'Breached') {
            if ('Notification' in window) {
              if (Notification.permission === 'granted') {
                new Notification('Yntra Vault Security Alert', {
                  body: `⚠️ CRITICAL: The password for "${target.title}" was leaked in a new data breach! Change it immediately.`,
                  requireInteraction: true,
                });
              } else if (Notification.permission !== 'denied') {
                Notification.requestPermission().then((p) => {
                  if (p === 'granted') {
                    new Notification('Yntra Vault Security Alert', {
                      body: `⚠️ CRITICAL: The password for "${target.title}" was leaked in a new data breach! Change it immediately.`,
                      requireInteraction: true,
                    });
                  }
                });
              }
            }

            addToast({
              message: `⚠️ CRITICAL SECURITY WARNING: The password for "${target.title}" was found in a new data leak! Change it immediately.`,
              type: 'error',
            });
          }

          setEntries((prev) =>
            prev.map((e) => (e.id === target.id ? { ...e, breachStatus: newStatus } : e))
          );
          setSelectedEntry((prev) => (prev?.id === target.id ? { ...prev, breachStatus: newStatus } : prev));

          await backend.updateEntryBreachStatus(target.id, newStatus);
          hasChangesToSave = true;
        } else {
          const newStatus = { type: 'Safe' as const, checked_at: new Date().toISOString() };
          setEntries((prev) =>
            prev.map((e) => (e.id === target.id ? { ...e, breachStatus: newStatus } : e))
          );
          setSelectedEntry((prev) => (prev?.id === target.id ? { ...prev, breachStatus: newStatus } : prev));
          await backend.updateEntryBreachStatus(target.id, newStatus);
          hasChangesToSave = true;
        }
      } catch (err) {
        console.error('Background breach check failed for entry', target.title, err);
      }

      if (active) {
        timeoutId = setTimeout(checkNext, 2000);
      }
    };

    timeoutId = setTimeout(checkNext, 6000);

    return () => {
      active = false;
      if (timeoutId) clearTimeout(timeoutId);
      if (hasChangesToSave && backend) {
        backend.saveVault().catch(() => {});
      }
    };
  }, [backend, isLocked, currentVault, entries.length, settings.autoBreachCheck, addToast]);

  const value = useMemo(
    () => ({
      entries,
      tags,
      selectedEntry,
      selectedEntryIds,
      isLoadingEntries,
      isLoadingDetail,
      setSelectedEntry,
      setSelectedEntryIds,
      toggleEntrySelection,
      clearSelection,
      bulkUpdateEntries,
      bulkDeleteEntries,
      updateEntry,
      updateBreachStatus,
      saveVault,
      addEntry,
      deleteEntry,
      deleteAttachment,
      toggleFavorite,
      togglePin,
      selectEntryById,
      addTag,
      updateTag,
      removeTag,
      reorderTags,
      refreshEntries,
      refreshTags,
      isP2pListening,
      toggleP2pListener,
      localLanIp,
    }),
    [
      entries,
      tags,
      selectedEntry,
      selectedEntryIds,
      isLoadingEntries,
      isLoadingDetail,
      setSelectedEntry,
      setSelectedEntryIds,
      toggleEntrySelection,
      clearSelection,
      bulkUpdateEntries,
      bulkDeleteEntries,
      updateEntry,
      updateBreachStatus,
      saveVault,
      addEntry,
      deleteEntry,
      deleteAttachment,
      toggleFavorite,
      togglePin,
      selectEntryById,
      addTag,
      updateTag,
      removeTag,
      reorderTags,
      refreshEntries,
      refreshTags,
      isP2pListening,
      toggleP2pListener,
      localLanIp,
    ]
  );

  return (
    <EntriesContext.Provider value={value}>
      {children}
    </EntriesContext.Provider>
  );
}

export function useEntries(): EntriesContextType {
  const ctx = useContext(EntriesContext);
  if (!ctx) throw new Error('useEntries must be used within EntriesProvider');
  return ctx;
}

// ─── Filtered Entries Hook ──────────────────────────────────────────────

export function useFilteredEntries(): PasswordEntry[] {
  const { entries } = useEntries();
  const { filterCategory } = useUi();
  const { searchTerm } = useSearch();
  const { settings } = useSettings();

  return useMemo(() => {
    let result = [...entries];

    if (filterCategory === 'favorites') {
      result = result.filter((e) => e.favorite);
    } else if (filterCategory !== 'all') {
      result = result.filter((e) => e.tags.includes(filterCategory));
    }

    if (searchTerm.trim()) {
      const term = searchTerm.toLowerCase();
      result = result.filter(
        (e) =>
          e.title.toLowerCase().includes(term) ||
          e.username.toLowerCase().includes(term) ||
          e.url.toLowerCase().includes(term) ||
          e.tags.some((t) => t.toLowerCase().includes(term))
      );
    }

    result.sort((a, b) => {
      if (a.pinned && !b.pinned) return -1;
      if (!a.pinned && b.pinned) return 1;

      const sortOrder = settings.entrySortOrder ?? 'updated';
      if (sortOrder === 'title') {
        return a.title.localeCompare(b.title);
      } else if (sortOrder === 'created') {
        const timeA = new Date(a.createdAt || a.updatedAt).getTime() || 0;
        const timeB = new Date(b.createdAt || b.updatedAt).getTime() || 0;
        return timeB - timeA;
      }
      const timeA = new Date(a.updatedAt || a.createdAt).getTime() || 0;
      const timeB = new Date(b.updatedAt || b.createdAt).getTime() || 0;
      return timeB - timeA;
    });

    return result;
  }, [entries, filterCategory, searchTerm, settings.entrySortOrder]);
}
