import { createContext, useContext, useState, useCallback, useMemo, useEffect, useRef } from 'react';
import type { PasswordEntry, Tag, Vault, FilterCategory, AppSettings, ToastMessage } from '@/types';
import { isTauri, getBackend, type YntraVaultBackend, type EntryPreview, type DecryptedEntry } from '@/lib/backend';



const DEFAULT_SETTINGS: AppSettings = {
  theme: 'system',
  sidebarWidth: 220,
  passwordListWidth: 320,
  fontSize: 13,
  density: 'normal',
  autoLockMinutes: 15,
  clipboardClearSeconds: 30,
  minimizeToTray: true,
  launchOnStartup: false,
  disableSkeletonDelays: false,
  autoBreachCheck: true,
  showBreachInList: true,
};

// ─── Conversion helpers (Rust types ↔ frontend types) ───────────────────

function entryPreviewToPasswordEntry(preview: EntryPreview, password = '••••••••'): PasswordEntry {
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
  };
}

function decryptedEntryToPasswordEntry(entry: DecryptedEntry): PasswordEntry {
  let recoveryCodes = '';
  const customFields = (entry.custom_fields || [])
    .filter((f) => {
      if (f.name === '2FA Recovery Codes') {
        recoveryCodes = f.value;
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
  };
}

// ─── Context Types ──────────────────────────────────────────────────────

interface AppStateContextType {
  entries: PasswordEntry[];
  filteredEntries: PasswordEntry[];
  tags: Tag[];
  selectedEntry: PasswordEntry | null;
  searchTerm: string;
  filterCategory: FilterCategory;
  settings: AppSettings;
  isEditing: boolean;
  settingsOpen: boolean;
  toasts: ToastMessage[];
  vaults: Vault[];
  currentVault: Vault | null;
  isLocked: boolean;
  backendReady: boolean;
  isLoadingEntries: boolean;
  isLoadingDetail: boolean;
  setSelectedEntry: (entry: PasswordEntry | null) => void;
  setSearchTerm: (term: string) => void;
  setFilterCategory: (cat: FilterCategory) => void;
  setIsEditing: (editing: boolean) => void;
  setSettingsOpen: (open: boolean) => void;
  updateEntry: (entry: PasswordEntry) => void;
  addEntry: (entry: PasswordEntry) => void;
  deleteEntry: (id: string) => void;
  updateSettings: (partial: Partial<AppSettings>) => void;
  addToast: (msg: Omit<ToastMessage, 'id'>) => void;
  removeToast: (id: string) => void;
  setCurrentVault: (vault: Vault | null) => void;
  setIsLocked: (locked: boolean) => void;
  toggleFavorite: (id: string) => void;
  togglePin: (id: string) => void;
  selectEntryById: (id: string) => void;
  addTag: (tag: Tag) => void;
  updateTag: (id: string, updates: Partial<Tag>) => void;
  removeTag: (id: string) => void;
  addVault: (vault: Vault) => void;
  removeVault: (id: string) => void;
  refreshEntries: () => Promise<void>;
  refreshTags: () => Promise<void>;
}

const AppStateContext = createContext<AppStateContextType | undefined>(undefined);

// ─── Provider ───────────────────────────────────────────────────────────

export function AppStateProvider({ children }: { children: React.ReactNode }) {
  const [backend, setBackend] = useState<YntraVaultBackend | null>(null);
  const [backendReady, setBackendReady] = useState(false);
  const [isLoadingEntries, setIsLoadingEntries] = useState(false);
  const [isLoadingDetail, setIsLoadingDetail] = useState(false);
  const [entries, setEntries] = useState<PasswordEntry[]>([]);
  const [rawTags, setRawTags] = useState<Tag[]>([]);
  const [selectedEntry, setSelectedEntry] = useState<PasswordEntry | null>(null);
  const [decryptedCache, setDecryptedCache] = useState<Record<string, PasswordEntry>>({});
  const [searchTerm, setSearchTerm] = useState('');
  const [filterCategory, setFilterCategory] = useState<FilterCategory>('all');
  const [settings, setSettings] = useState<AppSettings>(() => {
    try {
      const saved = localStorage.getItem('yntra-vault-settings');
      if (saved) {
        return { ...DEFAULT_SETTINGS, ...JSON.parse(saved) };
      }
    } catch (e) {
      console.warn('Failed to load settings from localStorage:', e);
    }
    return DEFAULT_SETTINGS;
  });
  const [isEditing, setIsEditing] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [toasts, setToasts] = useState<ToastMessage[]>([]);
  const [vaults, setVaults] = useState<Vault[]>([
    { id: '1', name: 'Personal Vault', path: '~/.yntra-vault/vault.db' },
    { id: '2', name: 'Work Vault', path: '~/.yntra-vault/work.db' },
  ]);
  const [currentVault, setCurrentVault] = useState<Vault | null>(null);
  const [isLocked, setIsLocked] = useState(false);

  const addToast = useCallback((msg: Omit<ToastMessage, 'id'>) => {
    const id = Math.random().toString(36).slice(2);
    setToasts((prev) => [...prev, { ...msg, id }]);
    setTimeout(() => {
      setToasts((prev) => prev.filter((t) => t.id !== id));
    }, 3000);
  }, []);

  const removeToast = useCallback((id: string) => {
    setToasts((prev) => prev.filter((t) => t.id !== id));
  }, []);

  // Initialize backend when running in Tauri
  useEffect(() => {
    if (isTauri()) {
      getBackend()
        .then((b) => {
          setBackend(b);
          setBackendReady(true);
        })
        .catch((e) => {
          console.warn('Backend init failed, using mock data:', e);
          setBackendReady(true);
        });
    } else {
      // Browser dev mode — use mock data
      setBackendReady(true);
    }
  }, []);

  // Load entries from backend when vault is opened
  const refreshEntries = useCallback(async () => {
    if (!backend) return;
    try {
      const previews = await backend.listEntries();
      setEntries(previews.map(p => entryPreviewToPasswordEntry(p)));
    } catch (e) {
      console.error('Failed to load entries:', e);
    }
  }, [backend]);

  // Load tags from backend when vault is opened
  const refreshTags = useCallback(async () => {
    if (!backend) return;
    try {
      const dbTags = await backend.getTags();
      setRawTags(dbTags.map(t => ({
        id: t.id,
        name: t.name,
        color: t.color || '#5b8def',
        icon: t.icon || 'tag',
        count: 0
      })));
    } catch (e) {
      console.error('Failed to load tags:', e);
    }
  }, [backend]);

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
              await new Promise(resolve => setTimeout(resolve, 200 - elapsed));
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
        setDecryptedCache({});
      }
    }
  }, [backend, currentVault, isLocked, refreshEntries, refreshTags, settings.disableSkeletonDelays]);

  // Fetch full entry details when selecting (Tauri mode)
  const selectEntryById = useCallback(async (id: string) => {
    // Check if details are already in the cache to avoid displaying skeleton loader repeatedly
    if (decryptedCache[id]) {
      setSelectedEntry(decryptedCache[id]);
      return;
    }

    setIsLoadingDetail(true);
    const startTime = Date.now();
    if (backend) {
      try {
        const full = await backend.getEntry(id);
        const entry = decryptedEntryToPasswordEntry(full);
        const elapsed = Date.now() - startTime;
        if (!settings.disableSkeletonDelays && elapsed < 150) {
          await new Promise(resolve => setTimeout(resolve, 150 - elapsed));
        }
        setDecryptedCache(prev => ({ ...prev, [id]: entry }));
        setSelectedEntry(entry);
      } catch (e) {
        // Fallback to list data
        setSelectedEntry(entries.find(e => e.id === id) || null);
      } finally {
        setIsLoadingDetail(false);
      }
    } else {
      // Mock mode - artificial delay for loading state transition
      if (!settings.disableSkeletonDelays) {
        await new Promise(resolve => setTimeout(resolve, 200));
      }
      const entry = entries.find(e => e.id === id) || null;
      if (entry) {
        setDecryptedCache(prev => ({ ...prev, [id]: entry }));
      }
      setSelectedEntry(entry);
      setIsLoadingDetail(false);
    }
  }, [backend, entries, decryptedCache, settings.disableSkeletonDelays]);

  // ─── Dynamic tag counts ─────────────────────────────────────────

  const tags = useMemo(() => {
    const countMap: Record<string, number> = {};
    for (const entry of entries) {
      for (const tagName of entry.tags) {
        countMap[tagName] = (countMap[tagName] || 0) + 1;
      }
    }
    return rawTags.map(tag => ({ ...tag, count: countMap[tag.name] || 0 }));
  }, [rawTags, entries]);

  // ─── Filtered + sorted entries ──────────────────────────────────

  const filteredEntries = useMemo(() => {
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
      return new Date(b.updatedAt).getTime() - new Date(a.updatedAt).getTime();
    });

    return result;
  }, [entries, filterCategory, searchTerm]);

  // ─── CRUD Operations (backend-aware) ────────────────────────────

  const updateEntry = useCallback(async (entry: PasswordEntry) => {
    if (backend) {
      try {
        const customFieldsToSend = [
          ...entry.customFields.map(f => ({
            id: f.id,
            name: f.name,
            field_type: (f.type.charAt(0).toUpperCase() + f.type.slice(1)) as any,
            value: f.value,
            sensitive: f.type === 'password',
          }))
        ];
        if (entry.recoveryCodes) {
          customFieldsToSend.push({
            id: crypto.randomUUID(),
            name: '2FA Recovery Codes',
            field_type: 'Password' as any,
            value: entry.recoveryCodes,
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

        await backend.updateEntry(entry.id, updatePayload);
        setDecryptedCache((prev) => {
          const next = { ...prev };
          delete next[entry.id];
          return next;
        });
        await refreshEntries();
      } catch (e) {
        addToast({ message: `Failed to update: ${e}`, type: 'error' });
      }
    } else {
      setEntries((prev) => prev.map((e) => (e.id === entry.id ? { ...entry, updatedAt: new Date().toISOString() } : e)));
      setDecryptedCache((prev) => ({ ...prev, [entry.id]: { ...entry, updatedAt: new Date().toISOString() } }));
    }
    setSelectedEntry((prev) => (prev?.id === entry.id ? { ...entry, updatedAt: new Date().toISOString() } : prev));
  }, [backend, refreshEntries]);

  const addEntry = useCallback(async (entry: PasswordEntry) => {
    if (backend) {
      try {
        const customFieldsToSend = entry.customFields.map(f => ({
          id: f.id,
          name: f.name,
          field_type: (f.type.charAt(0).toUpperCase() + f.type.slice(1)) as any,
          value: f.value,
          sensitive: f.type === 'password',
        }));
        if (entry.recoveryCodes) {
          customFieldsToSend.push({
            id: crypto.randomUUID(),
            name: '2FA Recovery Codes',
            field_type: 'Password' as any,
            value: entry.recoveryCodes,
            sensitive: true,
          });
        }

        await backend.addEntry({
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
        });
        await refreshEntries();
        addToast({ message: 'Entry created', type: 'success' });
      } catch (e) {
        addToast({ message: `Failed to add: ${e}`, type: 'error' });
      }
    } else {
      setEntries((prev) => [entry, ...prev]);
      setSelectedEntry(entry);
    }
  }, [backend, refreshEntries]);

  const deleteEntry = useCallback(async (id: string) => {
    if (backend) {
      try {
        await backend.deleteEntry(id);
        setDecryptedCache((prev) => {
          const next = { ...prev };
          delete next[id];
          return next;
        });
        await refreshEntries();
        addToast({ message: 'Moved to trash', type: 'info' });
      } catch (e) {
        addToast({ message: `Failed to delete: ${e}`, type: 'error' });
      }
    } else {
      setEntries((prev) => prev.filter((e) => e.id !== id));
      setDecryptedCache((prev) => {
        const next = { ...prev };
        delete next[id];
        return next;
      });
    }
    setSelectedEntry((prev) => (prev?.id === id ? null : prev));
  }, [backend, refreshEntries]);

  const toggleFavorite = useCallback(async (id: string) => {
    if (backend) {
      try {
        await backend.toggleFavorite(id);
        setDecryptedCache((prev) => {
          const next = { ...prev };
          delete next[id];
          return next;
        });
        await refreshEntries();
      } catch (e) {
        console.error('Toggle favorite failed:', e);
      }
    } else {
      setEntries((prev) =>
        prev.map((e) => (e.id === id ? { ...e, favorite: !e.favorite } : e))
      );
      setDecryptedCache((prev) => {
        if (prev[id]) {
          return { ...prev, [id]: { ...prev[id], favorite: !prev[id].favorite } };
        }
        return prev;
      });
    }
    setSelectedEntry((prev) => (prev?.id === id ? { ...prev, favorite: !prev.favorite } : prev));
  }, [backend, refreshEntries]);

  const togglePin = useCallback(async (id: string) => {
    if (backend) {
      try {
        await backend.togglePin(id);
        setDecryptedCache((prev) => {
          const next = { ...prev };
          delete next[id];
          return next;
        });
        await refreshEntries();
      } catch (e) {
        console.error('Toggle pin failed:', e);
      }
    } else {
      setEntries((prev) =>
        prev.map((e) => (e.id === id ? { ...e, pinned: !e.pinned } : e))
      );
      setDecryptedCache((prev) => {
        if (prev[id]) {
          return { ...prev, [id]: { ...prev[id], pinned: !prev[id].pinned } };
        }
        return prev;
      });
    }
    setSelectedEntry((prev) => (prev?.id === id ? { ...prev, pinned: !prev.pinned } : prev));
  }, [backend, refreshEntries]);

  // ─── Tag CRUD ───────────────────────────────────────────────────

  const addTag = useCallback(async (tag: Tag) => {
    if (backend) {
      try {
        await backend.addTag(tag.name, tag.color, tag.icon);
        await refreshTags();
      } catch (e) {
        addToast({ message: `Failed to create tag: ${e}`, type: 'error' });
      }
    } else {
      setRawTags((prev) => [...prev, tag]);
    }
  }, [backend, refreshTags, addToast]);

  const updateTag = useCallback((id: string, updates: Partial<Tag>) => {
    setRawTags((prev) => {
      const oldTag = prev.find((t) => t.id === id);
      // If name changed, update all entries that reference the old name
      if (oldTag && updates.name && oldTag.name !== updates.name) {
        const oldName = oldTag.name;
        const newName = updates.name;
        setEntries((entries) =>
          entries.map((e) => ({
            ...e,
            tags: e.tags.map((t) => (t === oldName ? newName : t)),
          }))
        );
      }
      return prev.map((t) => (t.id === id ? { ...t, ...updates } : t));
    });
  }, []);

  const removeTag = useCallback(async (id: string) => {
    if (backend) {
      try {
        await backend.deleteTag(id);
        await refreshTags();
      } catch (e) {
        addToast({ message: `Failed to delete tag: ${e}`, type: 'error' });
      }
    } else {
      setRawTags((prev) => {
        const tag = prev.find((t) => t.id === id);
        if (tag) {
          // Remove tag from all entries
          setEntries((entries) =>
            entries.map((e) => ({
              ...e,
              tags: e.tags.filter((t) => t !== tag.name),
            }))
          );
        }
        return prev.filter((t) => t.id !== id);
      });
    }
  }, [backend, refreshTags, addToast]);

  // ─── Vault CRUD ─────────────────────────────────────────────────

  const addVault = useCallback((vault: Vault) => {
    setVaults((prev) => [...prev, vault]);
  }, []);

  const removeVault = useCallback((id: string) => {
    setVaults((prev) => prev.filter((v) => v.id !== id));
  }, []);

  // ─── Settings ───────────────────────────────────────────────────

   const updateSettings = useCallback((partial: Partial<AppSettings>) => {
    setSettings((prev) => {
      const next = { ...prev, ...partial };
      try {
        localStorage.setItem('yntra-vault-settings', JSON.stringify(next));
      } catch (e) {
        console.warn('Failed to save settings to localStorage:', e);
      }
      return next;
    });
  }, []);

  const entriesRef = useRef(entries);
  useEffect(() => {
    entriesRef.current = entries;
  }, [entries]);

  // Silent background vault-wide breach check on vault unlock
  useEffect(() => {
    if (!backend || isLocked || !currentVault || entries.length === 0 || !settings.autoBreachCheck) {
      return;
    }

    let active = true;
    let timeoutId: any = null;

    const checkNext = async () => {
      if (!active) return;

      // Find first entry that has e.breachStatus.type === 'Unknown' (or is undefined)
      const target = entriesRef.current.find(e => !e.breachStatus || e.breachStatus.type === 'Unknown');

      if (!target) {
        // No more unchecked entries!
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
            // New breach alert! Native Windows notification + Toast error
            if ('Notification' in window) {
              if (Notification.permission === 'granted') {
                new Notification('Yntra Vault Security Alert', {
                  body: `⚠️ CRITICAL: The password for "${target.title}" was leaked in a new data breach! Change it immediately.`,
                  requireInteraction: true,
                });
              } else if (Notification.permission !== 'denied') {
                Notification.requestPermission().then(p => {
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

          // Update locally
          setEntries(prev => prev.map(e => e.id === target.id ? { ...e, breachStatus: newStatus } : e));
          
          setSelectedEntry(prev => prev?.id === target.id ? { ...prev, breachStatus: newStatus } : prev);

          // Update backend database silently
          const customFieldsToSend = [
            ...decrypted.customFields.map(f => ({
              id: f.id,
              name: f.name,
              field_type: (f.type.charAt(0).toUpperCase() + f.type.slice(1)) as any,
              value: f.value,
              sensitive: f.type === 'password',
            }))
          ];
          if (decrypted.recoveryCodes) {
            customFieldsToSend.push({
              id: crypto.randomUUID(),
              name: '2FA Recovery Codes',
              field_type: 'Password' as any,
              value: decrypted.recoveryCodes,
              sensitive: true,
            });
          }

          const updatePayload: any = {
            title: decrypted.title,
            username: decrypted.username,
            url: decrypted.url,
            email: decrypted.email,
            notes: decrypted.notes,
            tags: decrypted.tags,
            favorite: decrypted.favorite,
            pinned: decrypted.pinned,
            custom_fields: customFieldsToSend,
            breach_status: newStatus,
          };

          if (decrypted.totpSecret !== undefined && decrypted.totpSecret !== 'has-totp') {
            updatePayload.totp_secret = decrypted.totpSecret;
          }

          await backend.updateEntry(target.id, updatePayload);
        } else {
          // If entry has no password, mark it as safe silently
          const newStatus = { type: 'Safe' as const, checked_at: new Date().toISOString() };
          setEntries(prev => prev.map(e => e.id === target.id ? { ...e, breachStatus: newStatus } : e));
          setSelectedEntry(prev => prev?.id === target.id ? { ...prev, breachStatus: newStatus } : prev);
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
    };
  }, [backend, isLocked, currentVault, entries.length, settings.autoBreachCheck]);

  return (
    <AppStateContext.Provider
      value={{
        entries,
        filteredEntries,
        tags,
        selectedEntry,
        searchTerm,
        filterCategory,
        settings,
        isEditing,
        settingsOpen,
        toasts,
        vaults,
        currentVault,
        isLocked,
        backendReady,
        isLoadingEntries,
        isLoadingDetail,
        setSelectedEntry,
        setSearchTerm,
        setFilterCategory,
        setIsEditing,
        setSettingsOpen,
        updateEntry,
        addEntry,
        deleteEntry,
        updateSettings,
        addToast,
        removeToast,
        setCurrentVault,
        setIsLocked,
        toggleFavorite,
        togglePin,
        selectEntryById,
        addTag,
        updateTag,
        removeTag,
        addVault,
        removeVault,
        refreshEntries,
        refreshTags,
      }}
    >
      {children}
    </AppStateContext.Provider>
  );
}

export function useAppState() {
  const ctx = useContext(AppStateContext);
  if (!ctx) throw new Error('useAppState must be used within AppStateProvider');
  return ctx;
}

