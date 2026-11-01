import { useToast } from '@/contexts/ToastContext';
import { useAuth } from '@/features/auth';
import { useSettings } from '@/features/settings';
import { getTranslation } from '@/i18n/translations';
import type { BreachStatus, FieldType, UpdateEntry } from '@/lib/backend';
import { sendDesktopNotification } from '@/lib/notifications';
import { getTransientWebdavPassword } from '@/lib/sessionSecrets';
import type { PasswordEntry,Tag } from '@/types';
import React,{ useCallback,useEffect,useMemo,useRef,useState } from 'react';
import { decryptedEntryToPasswordEntry,EntriesContext,entryPreviewToPasswordEntry,isRecoveryField,needsBreachRecheck,passwordEntryToPreview } from './EntriesContext.model';


export function EntriesProvider({ children }: { children: React.ReactNode }) {
  const { currentVault, isLocked, getSessionGeneration } = useAuth();
  const listenerTailRef = useRef<Promise<void>>(Promise.resolve());
  const [listenerManualOverride, setListenerManualOverride] = useState<boolean | null>(null);
  const scope = String(isLocked) + ':' + (currentVault?.id ?? '') + ':' + (currentVault?.path ?? '') + ':' + getSessionGeneration();
  const [previousScope, setPreviousScope] = useState({ scope, locked: isLocked });
  if (previousScope.scope !== scope) {
    setPreviousScope({ scope, locked: isLocked });
    if (isLocked || !previousScope.locked) setListenerManualOverride(null);
  }
  return <EntriesSession listenerTailRef={listenerTailRef} listenerManualOverride={listenerManualOverride} setListenerManualOverride={setListenerManualOverride} key={scope}>{children}</EntriesSession>;
}
function EntriesSession({ children, listenerTailRef, listenerManualOverride, setListenerManualOverride }: { children: React.ReactNode; listenerTailRef: React.RefObject<Promise<void>>; listenerManualOverride: boolean | null; setListenerManualOverride: React.Dispatch<React.SetStateAction<boolean | null>> }) {
  const { backend, currentVault, setCurrentVault, isLocked, getSessionGeneration, isSessionCurrent } = useAuth();
  const { settings, networkAccessReady } = useSettings();
  const { addToast } = useToast();

  const [isLoadingEntries, setIsLoadingEntries] = useState(() => Boolean(backend && currentVault && !isLocked));
  const [isP2pListening, setIsP2pListening] = useState(false);
  const [localLanIp, setLocalLanIp] = useState<string | null>(null);
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
        } catch { /* The focused element may have been removed during locking. */ }
      }


      if (backend) {
        backend.lockVault().catch((err) => {
          console.error('Failed to lock backend vault on lock state change:', err);
        });
      }
    }
  }, [isLocked, backend]);

  // Load entries from backend when vault is opened
  const refreshEntries = useCallback(async () => {
    const session = getSessionGeneration();
    if (!backend || !currentVault || !isSessionCurrent(session)) return;
    const request = ++refreshSeqRef.current;
    try {
      const previews = await backend.listEntries();
      if (!isSessionCurrent(session) || request !== refreshSeqRef.current) return;
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
            if (isSessionCurrent(session) && request === refreshSeqRef.current && selection === selectionSeqRef.current && selectedEntryRef.current?.id === currentSelected.id) {
              setSelectedEntry(decryptedEntryToPasswordEntry(full));
            }
          } catch (err) {
            if (isSessionCurrent(session) && request === refreshSeqRef.current && selection === selectionSeqRef.current) { setSelectedEntry(null); addToast({ message: String(err), type: 'error' }); }
          }
        }
      }
    } catch (e) {
      if (isSessionCurrent(session) && request === refreshSeqRef.current) addToast({ message: String(e), type: 'error' });
    }
  }, [backend, currentVault, getSessionGeneration, isSessionCurrent, addToast]);

  useEffect(() => () => {
    refreshSeqRef.current++;
    tagRefreshSeqRef.current++;
    selectionSeqRef.current++;
  }, [currentVault?.id, currentVault?.path]);

  // Load tags from backend when vault is opened
  const refreshTags = useCallback(async () => {
    const session = getSessionGeneration();
    if (!backend || !currentVault || !isSessionCurrent(session)) return;
    const request = ++tagRefreshSeqRef.current;
    try {
      const dbTags = await backend.getTags();
      if (!isSessionCurrent(session) || request !== tagRefreshSeqRef.current) return;
      setRawTags(
        dbTags.map((t) => ({
          id: t.id,
          name: t.name,
          color: t.color || '#5b8def',
          icon: t.icon || 'tag',
            default_url: t.default_url ?? '',
          count: 0,
        }))
      );
    } catch (e) {
      if (isSessionCurrent(session) && request === tagRefreshSeqRef.current) addToast({ message: String(e), type: 'error' });
    }
  }, [backend, currentVault, getSessionGeneration, isSessionCurrent, addToast]);

  // Auto-refresh when vault unlocks or changes
  useEffect(() => {
    if (backend) {
      if (currentVault && !isLocked) {
        const loadAll = async () => {
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
      }
    }
  }, [backend, currentVault, isLocked, refreshEntries, refreshTags, settings.disableSkeletonDelays]);

  // Fetch full entry details when selecting (Tauri mode)
  const selectEntryById = useCallback(
    async (id: string | null) => {
      const seq = ++selectionSeqRef.current;
      const session = getSessionGeneration();

      if (!id || !isSessionCurrent(session)) {
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
          if (seq !== selectionSeqRef.current || !isSessionCurrent(session)) {
            return;
          }

          const entry = decryptedEntryToPasswordEntry(full);
          if (!isSameEntry && !settings.disableSkeletonDelays) {
            const elapsed = Date.now() - startTime;
            if (elapsed < 150) {
              await new Promise((resolve) => setTimeout(resolve, 150 - elapsed));
            }
            if (seq !== selectionSeqRef.current || !isSessionCurrent(session)) {
              return;
            }
          }
          setSelectedEntry(entry);
        } catch (error) {
          if (seq === selectionSeqRef.current && isSessionCurrent(session)) {
            setSelectedEntry(null);
            addToast({ message: String(error), type: 'error' });
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
        if (seq !== selectionSeqRef.current || !isSessionCurrent(session)) {
          return;
        }
        const entry = entries.find((e) => e.id === id) || null;
        setSelectedEntry(entry);
        if (!isSameEntry && seq === selectionSeqRef.current) {
          setIsLoadingDetail(false);
        }
      }
    },
    [backend, entries, settings.disableSkeletonDelays, getSessionGeneration, isSessionCurrent, addToast]
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

  useEffect(() => {
    if (!backend?.onVaultContentChanged || isLocked) return;
    let disposed = false;
    let unsubscribe: (() => void) | undefined;
    const generation = getSessionGeneration();
    void backend.onVaultContentChanged(() => {
      if (!disposed && isSessionCurrent(generation)) {
        void Promise.all([refreshEntries(), refreshTags()]);
        void backend.getVaultInfo().then(info => {
          if (info && !disposed && isSessionCurrent(generation) && currentVault?.id === info.id && currentVault.name !== info.name) setCurrentVault({ ...currentVault, name: info.name });
        }).catch(error => { if (!disposed && isSessionCurrent(generation)) addToast({ message: String(error), type: 'error' }); });
      }
    }).then(stop => { if (disposed) stop(); else unsubscribe = stop; }).catch(() => {});
    return () => { disposed = true; unsubscribe?.(); };
  }, [backend, isLocked, currentVault, getSessionGeneration, isSessionCurrent, refreshEntries, refreshTags, setCurrentVault, addToast]);

  // Debounced auto-sync trigger for WebDAV cloud sync on changes
  const autoSyncTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const triggerAutoSync = useCallback(() => {
    if (settings.operationMode === 'airgap' || !settings.webdavEnabled || !settings.webdavAutoSync || !settings.webdavUrl || !settings.webdavUser || !backend) {
      return;
    }
    if (autoSyncTimerRef.current) {
      clearTimeout(autoSyncTimerRef.current);
    }
    const session = getSessionGeneration();
    autoSyncTimerRef.current = setTimeout(async () => {
      if (!isSessionCurrent(session)) return;
      try {
        const pass = getTransientWebdavPassword();
        const stats = await backend.webdavSync(settings.webdavUrl!, settings.webdavUser!, pass, true);
        if (stats && (stats.entries_added > 0 || stats.entries_updated > 0 || stats.tags_merged > 0 || stats.trash_merged > 0)) {
          await Promise.all([refreshEntries(), refreshTags()]);
        }
      } catch (err) {
        console.warn('Auto-sync on save failed:', err);
      }
    }, 2000);
  }, [settings.operationMode, settings.webdavEnabled, settings.webdavAutoSync, settings.webdavUrl, settings.webdavUser, backend, refreshEntries, refreshTags, getSessionGeneration, isSessionCurrent]);

  useEffect(() => () => {
    if (autoSyncTimerRef.current) clearTimeout(autoSyncTimerRef.current);
  }, [isLocked, currentVault?.id, currentVault?.path, settings.operationMode]);

  const toggleP2pListener = useCallback((enable?: boolean) => {
    setListenerManualOverride((prev) => {
      const current = prev !== null ? prev : Boolean(settings.p2pAutoListen);
      return enable !== undefined ? enable : !current;
    });
  }, [settings.p2pAutoListen, setListenerManualOverride]);

  // Fetch local LAN IP
  useEffect(() => {
    if (backend) {
      backend.getLocalIp().then((ip) => setLocalLanIp(ip || null)).catch(() => {});
    }
  }, [backend]);

  const shouldListen = networkAccessReady && settings.operationMode !== 'airgap' && (listenerManualOverride !== null ? listenerManualOverride : Boolean(settings.p2pAutoListen)) && !isLocked && Boolean(backend) && Boolean(currentVault);

  const listenerView = useRef({ refreshEntries, refreshTags, addToast, language: settings.language });
  useEffect(() => { listenerView.current = { refreshEntries, refreshTags, addToast, language: settings.language }; }, [refreshEntries, refreshTags, addToast, settings.language]);

  const listenerPath = currentVault?.path;
  // P2P Auto-Listener / Background loop
  useEffect(() => {
    if (!shouldListen || !backend || !listenerPath || isLocked) {
      return;
    }

    let isCancelled = false;
    const previous = listenerTailRef.current;

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
          const stats = await backend.runP2pSyncListener(addr, listenerPath);
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
    listenerTailRef.current = running;
    return () => {
      isCancelled = true;
      const stopping = backend.cancelP2pSyncListener().catch(() => {});
      listenerTailRef.current = Promise.all([running, stopping]).then(() => {});
      setIsP2pListening(false);
    };
  }, [shouldListen, settings.p2pAddr, backend, listenerPath, isLocked, listenerTailRef]);

  // P2P Auto-Sync on Wi-Fi (Periodically scans for beacons and syncs)
  useEffect(() => {
    if (!networkAccessReady || settings.operationMode === 'airgap' || !settings.p2pAutoSyncWifi || !backend || !currentVault || isLocked) {
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
          const stats = await backend.runP2pSyncClient(peer, currentVault.path, undefined, true);
          await Promise.all([refreshEntries(), refreshTags()]);
          const lang = settings.language || 'en';
          const count = (stats?.entries_added || 0) + (stats?.entries_updated || 0);
          const msg = count > 0
            ? getTranslation(lang, 'sync.toast_auto_sync_changes', { count: String(count) })
            : getTranslation(lang, 'sync.toast_auto_sync_clean');
          addToast({ message: msg, type: 'success' });
          sendDesktopNotification(getTranslation(lang, 'sync.desktop_title'), msg);
        }
      } catch (err) {
        const errStr = String((err instanceof Error ? err.message : String(err)) || err || '');
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
  }, [networkAccessReady, settings.operationMode, settings.p2pAutoSyncWifi, settings.p2pAutoSyncIntervalMinutes, settings.language, backend, currentVault, isLocked, refreshEntries, refreshTags, addToast]);

  // CRUD Operations
  // CRUD Operations with SOTA Optimistic Local-First State & Background Persistence
  const updateEntry = useCallback(
    async (entry: PasswordEntry) => {
      const session = getSessionGeneration();
      if (!isSessionCurrent(session)) return false;
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

      if (backend) {
        try {
          const customFieldsToSend = [
            ...cleanedCustomFields.map((f) => ({
              id: f.id,
              name: f.name,
              field_type: (f.type.charAt(0).toUpperCase() + f.type.slice(1)) as FieldType,
              value: f.value,
              sensitive: f.sensitive ?? f.type === 'password',
            })),
          ];
          if (recoveryCodes && recoveryCodes.trim()) {
            customFieldsToSend.push({
              id: crypto.randomUUID(),
              name: '2FA Recovery Codes',
              field_type: 'Password' as FieldType,
              value: recoveryCodes.trim(),
              sensitive: true,
            });
          }

          const updatePayload: UpdateEntry = {
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
          if (!isSessionCurrent(session)) return false;

          setEntries(previous => previous.map(item => item.id === entry.id ? passwordEntryToPreview(updatedEntry) : item));
          setSelectedEntry(previous => previous?.id === entry.id ? updatedEntry : previous);
          // If attachments or passkeys were staged, fetch refreshed entry metadata without reloading whole DB
          if (entry.newAttachments?.length || entry.deleteAttachmentIds?.length || entry.passkeyAction) {
            const updatedDetail = await backend.getEntry(entry.id).catch(() => null);
            if (!isSessionCurrent(session)) return false;
            if (updatedDetail) {
              const fullEntry = decryptedEntryToPasswordEntry(updatedDetail);
              setSelectedEntry((prev) => (prev?.id === entry.id ? fullEntry : prev));
              setEntries((prev) =>
                prev.map((e) => (e.id === entry.id ? passwordEntryToPreview(fullEntry) : e))
              );
            }
          }
          triggerAutoSync();
        } catch (e) {
          if (!isSessionCurrent(session)) return false;
          addToast({
            message: getTranslation(settings.language, 'toast.update_failed', { err: String(e) }),
            type: 'error',
          });
          return false;
        }
      } else {
        setEntries(previous => previous.map(item => item.id === entry.id ? passwordEntryToPreview(updatedEntry) : item));
        setSelectedEntry(previous => previous?.id === entry.id ? updatedEntry : previous);
      }
      return true;
    },
    [backend, settings.language, addToast, triggerAutoSync, getSessionGeneration, isSessionCurrent]
  );

  const updateBreachStatus = useCallback(
    async (id: string, newStatus: BreachStatus) => {
      if (!isSessionCurrent(getSessionGeneration())) return;
      setEntries((prev) => prev.map((e) => (e.id === id ? { ...e, breachStatus: newStatus } : e)));
      setSelectedEntry((prev) => (prev?.id === id ? { ...prev, breachStatus: newStatus } : prev));
      if (backend) {
        await backend.updateEntryBreachStatus(id, newStatus);
      }
    },
    [backend, getSessionGeneration, isSessionCurrent]
  );

  const saveVault = useCallback(async () => {
    const session = getSessionGeneration();
    if (!isSessionCurrent(session)) return;
    if (backend) {
      await backend.saveVault();
      if (isSessionCurrent(session)) triggerAutoSync();
    }
  }, [backend, triggerAutoSync, getSessionGeneration, isSessionCurrent]);

  const addEntry = useCallback(
    async (entry: PasswordEntry) => {
      const session = getSessionGeneration();
      if (!isSessionCurrent(session)) return false;
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
        field_type: (f.type.charAt(0).toUpperCase() + f.type.slice(1)) as FieldType,
        value: f.value,
        sensitive: f.sensitive ?? f.type === 'password',
      }));
      if (recoveryCodes && recoveryCodes.trim()) {
        customFieldsToSend.push({
          id: crypto.randomUUID(),
          name: '2FA Recovery Codes',
          field_type: 'Password' as FieldType,
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
          if (!isSessionCurrent(session)) return false;

          // In-memory instant insertion without full DB reload across IPC
          const createdEntry: PasswordEntry = {
            ...entry,
            id: newId,
            createdAt: entry.createdAt || now,
            updatedAt: entry.updatedAt || now,
            attachmentCount: (entry.newAttachments?.length || 0) + (entry.attachments?.length || 0),
          };

          setEntries((prev) => [passwordEntryToPreview(createdEntry), ...prev.filter((e) => e.id !== newId)]);
          setSelectedEntry(createdEntry);
          triggerAutoSync();
        } catch (e) {
          if (!isSessionCurrent(session)) return false;
          addToast({
            message: getTranslation(settings.language, 'toast.add_failed', { err: String(e) }),
            type: 'error',
          });
          return false;
        }
      } else {
        const localEntry: PasswordEntry = {
          ...entry,
          createdAt: entry.createdAt || now,
          updatedAt: entry.updatedAt || now,
        };
        setEntries((prev) => [passwordEntryToPreview(localEntry), ...prev]);
        setSelectedEntry(localEntry);
      }
      return true;
    },
    [backend, settings.language, addToast, triggerAutoSync, getSessionGeneration, isSessionCurrent]
  );

  const deleteEntry = useCallback(async (id: string) => {
    const session = getSessionGeneration();
    if (!isSessionCurrent(session)) return false;
    try {
      if (backend) await backend.deleteEntry(id);
      if (!isSessionCurrent(session)) return false;
      setEntries(previous => previous.filter(entry => entry.id !== id));
      setSelectedEntry(previous => previous?.id === id ? null : previous);
      setSelectedEntryIds(previous => previous.filter(selected => selected !== id));
      addToast({ message: getTranslation(settings.language, 'toast.moved_to_trash'), type: 'info' });
      triggerAutoSync(); return true;
    } catch (error) {
      if (isSessionCurrent(session)) addToast({ message: getTranslation(settings.language, 'toast.delete_failed', { err: String(error) }), type: 'error' });
      return false;
    }
  }, [backend, settings.language, addToast, triggerAutoSync, getSessionGeneration, isSessionCurrent]);

  const deleteAttachment = useCallback(async (entryId: string, attachmentId: string) => {
    const session = getSessionGeneration();
    if (!isSessionCurrent(session)) return;
    try {
      if (backend) await backend.deleteAttachment(entryId, attachmentId);
      if (!isSessionCurrent(session)) return;
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


      triggerAutoSync();
    } catch (error) {
      if (isSessionCurrent(session)) addToast({ message: getTranslation(settings.language, 'toast.delete_failed', { err: String(error) }), type: 'error' });
    }
  }, [backend, settings.language, addToast, triggerAutoSync, getSessionGeneration, isSessionCurrent]);

  const toggleFavorite = useCallback(async (id: string) => {
    const session = getSessionGeneration();
    if (!isSessionCurrent(session)) return;
    try {
      const confirmed = backend ? await backend.toggleFavorite(id) : undefined;
      if (!isSessionCurrent(session)) return;
      setEntries(previous => previous.map(entry => entry.id === id ? { ...entry, favorite: confirmed ?? !entry.favorite } : entry));
      setSelectedEntry(previous => previous?.id === id ? { ...previous, favorite: confirmed ?? !previous.favorite } : previous);
      triggerAutoSync();
    } catch (error) {
      if (isSessionCurrent(session)) addToast({ message: getTranslation(settings.language, 'toast.action_failed', { err: String(error) }), type: 'error' });
    }
  }, [backend, settings.language, addToast, triggerAutoSync, getSessionGeneration, isSessionCurrent]);

  const togglePin = useCallback(async (id: string) => {
    const session = getSessionGeneration();
    if (!isSessionCurrent(session)) return;
    try {
      const confirmed = backend ? await backend.togglePin(id) : undefined;
      if (!isSessionCurrent(session)) return;
      setEntries(previous => previous.map(entry => entry.id === id ? { ...entry, pinned: confirmed ?? !entry.pinned } : entry));
      setSelectedEntry(previous => previous?.id === id ? { ...previous, pinned: confirmed ?? !previous.pinned } : previous);
      triggerAutoSync();
    } catch (error) {
      if (isSessionCurrent(session)) addToast({ message: getTranslation(settings.language, 'toast.action_failed', { err: String(error) }), type: 'error' });
    }
  }, [backend, settings.language, addToast, triggerAutoSync, getSessionGeneration, isSessionCurrent]);

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

  const bulkUpdateEntries = useCallback(async (ids: string[], updates: Partial<PasswordEntry>, tagsToAdd: string[] = [], tagsToRemove: string[] = [], notesMode: 'append' | 'overwrite' = 'append') => {
    const session = getSessionGeneration();
    if (!isSessionCurrent(session)) return false;
    try {
      if (backend) {
        await backend.bulkUpdateEntries(ids, { title: updates.title, username: updates.username, url: updates.url, favorite: updates.favorite, pinned: updates.pinned, notes: updates.notes, append_notes: notesMode === 'append', tags_to_add: tagsToAdd, tags_to_remove: tagsToRemove });
        if (!isSessionCurrent(session)) return false;
        await refreshEntries();
        if (!isSessionCurrent(session)) return false;
      } else {
        const apply = (entry: PasswordEntry) => ids.includes(entry.id) ? { ...entry, ...updates, tags: [...new Set([...entry.tags, ...tagsToAdd])].filter(tag => !tagsToRemove.includes(tag)), notes: updates.notes === undefined ? entry.notes : notesMode === 'append' && entry.notes ? entry.notes + '\n' + updates.notes : updates.notes } : entry;
        setEntries(previous => previous.map(entry => passwordEntryToPreview(apply(entry))));
        setSelectedEntry(previous => previous ? apply(previous) : null);
      }
      addToast({ message: getTranslation(settings.language, 'toast.bulk_updated', { count: ids.length }), type: 'info' });
      triggerAutoSync(); return true;
    } catch (error) {
      if (isSessionCurrent(session)) addToast({ message: getTranslation(settings.language, 'toast.action_failed', { err: String(error) }), type: 'error' });
      return false;
    }
  }, [backend, refreshEntries, settings.language, addToast, triggerAutoSync, getSessionGeneration, isSessionCurrent]);

  const bulkDeleteEntries = useCallback(async (ids: string[]) => {
    const session = getSessionGeneration();
    if (!isSessionCurrent(session)) return false;
    try {
      if (backend) await backend.bulkDeleteEntries(ids);
      if (!isSessionCurrent(session)) return false;
      setEntries(previous => previous.filter(entry => !ids.includes(entry.id)));
      setSelectedEntry(previous => previous && ids.includes(previous.id) ? null : previous);
      setSelectedEntryIds(previous => previous.filter(id => !ids.includes(id)));
      addToast({ message: getTranslation(settings.language, 'toast.moved_to_trash'), type: 'info' });
      triggerAutoSync(); return true;
    } catch (error) {
      if (isSessionCurrent(session)) addToast({ message: String(error), type: 'error' });
      return false;
    }
  }, [backend, settings.language, addToast, triggerAutoSync, getSessionGeneration, isSessionCurrent]);

  const addTag = useCallback(async (tag: Tag) => {
    const session = getSessionGeneration();
    if (!isSessionCurrent(session)) return false;
    try {
      const id = backend ? await backend.addTag(tag.name, tag.color, tag.icon, tag.default_url) : tag.id;
      if (!isSessionCurrent(session)) return false;
      setRawTags(previous => [...previous.filter(item => item.id !== id), { ...tag, id }]);
      triggerAutoSync();
      return true;
    } catch (error) {
      if (isSessionCurrent(session)) addToast({ message: String(error), type: "error" });
      return false;
    }
  }, [backend, addToast, triggerAutoSync, getSessionGeneration, isSessionCurrent]);

  const updateTag = useCallback(async (id: string, updates: Partial<Tag>) => {
    const session = getSessionGeneration();
    const oldTag = rawTags.find(tag => tag.id === id);
    if (!isSessionCurrent(session) || !oldTag) return false;
    const next = { ...oldTag, ...updates };
    try {
      if (backend) await backend.updateTag(id, next.name, next.color, next.icon, next.default_url);
      if (!isSessionCurrent(session)) return false;
      setRawTags(previous => previous.map(tag => tag.id === id ? next : tag));
      setEntries(previous => previous.map(entry => ({ ...entry, tags: entry.tags.map(name => name === oldTag.name ? next.name : name) })));
      setSelectedEntry(previous => previous ? { ...previous, tags: previous.tags.map(name => name === oldTag.name ? next.name : name) } : null);
      triggerAutoSync();
      return true;
    } catch (error) {
      if (isSessionCurrent(session)) addToast({ message: String(error), type: "error" });
      return false;
    }
  }, [backend, rawTags, addToast, triggerAutoSync, getSessionGeneration, isSessionCurrent]);

  const removeTag = useCallback(async (id: string) => {
    const session = getSessionGeneration();
    const tag = rawTags.find(tag => tag.id === id);
    if (!isSessionCurrent(session) || !tag) return false;
    try {
      if (backend) await backend.deleteTag(id);
      if (!isSessionCurrent(session)) return false;
      setRawTags(previous => previous.filter(tag => tag.id !== id));
      setEntries(previous => previous.map(entry => ({ ...entry, tags: entry.tags.filter(name => name !== tag.name) })));
      setSelectedEntry(previous => previous ? { ...previous, tags: previous.tags.filter(name => name !== tag.name) } : null);
      triggerAutoSync();
      return true;
    } catch (error) {
      if (isSessionCurrent(session)) addToast({ message: String(error), type: "error" });
      return false;
    }
  }, [backend, rawTags, addToast, triggerAutoSync, getSessionGeneration, isSessionCurrent]);

  const reorderTags = useCallback(
    async (newTags: Tag[]) => {
      const session = getSessionGeneration();
      if (!isSessionCurrent(session)) return;
      const prevTags = rawTagsRef.current;
      setRawTags(newTags);

      if (backend) {
        try {
          await backend.reorderTags(newTags.map((t) => t.id));
          if (!isSessionCurrent(session)) return;
          triggerAutoSync();
        } catch (e) {
          if (!isSessionCurrent(session)) return;
          setRawTags(prevTags);
          addToast({ message: `Failed to save tag order: ${e}`, type: 'error' });
        }
      }
    },
    [backend, addToast, triggerAutoSync, getSessionGeneration, isSessionCurrent]
  );

  // Silent background vault-wide breach check on vault unlock
  const entriesRef = useRef(entries);
  useEffect(() => {
    entriesRef.current = entries;
  }, [entries]);

  useEffect(() => {
    if (!networkAccessReady || !backend || isLocked || !currentVault || entries.length === 0 || !settings.autoBreachCheck || settings.operationMode === 'airgap') {
      return;
    }

    let active = true;
    let timeoutId: ReturnType<typeof setTimeout> | null = null;
    let hasChangesToSave = false;
    const session = getSessionGeneration();
    const attempted = new Set<string>();

    const checkNext = async () => {
      if (!active || !isSessionCurrent(session)) return;

      const target = entriesRef.current.find((e) => !attempted.has(e.id) && needsBreachRecheck(e.breachStatus));

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
        attempted.add(target.id);
        const decryptedRaw = await backend.getEntry(target.id);
        const decrypted = decryptedEntryToPasswordEntry(decryptedRaw);
        if (!active || !isSessionCurrent(session)) return;

        const passwordValue = decrypted.password;
        if (passwordValue && passwordValue.trim() !== '') {
          const result = await backend.checkPasswordBreach(passwordValue);
          if (!active || !isSessionCurrent(session)) return;

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

      if (active && isSessionCurrent(session)) {
        timeoutId = setTimeout(checkNext, 2000);
      }
    };

    timeoutId = setTimeout(checkNext, 6000);

    return () => {
      active = false;
      if (timeoutId) clearTimeout(timeoutId);
      if (hasChangesToSave && backend && isSessionCurrent(session)) {
        backend.saveVault().catch(() => {});
      }
    };
  }, [networkAccessReady, backend, isLocked, currentVault, entries.length, settings.autoBreachCheck, settings.operationMode, addToast, getSessionGeneration, isSessionCurrent]);

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
      p2pReceiverEnabled: Boolean(shouldListen),
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
      shouldListen,
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
