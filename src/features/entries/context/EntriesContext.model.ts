import { useSearch,useUi } from '@/contexts/UiContext';
import { useSettings } from '@/features/settings';
import type { BreachStatus,DecryptedEntry,EntryPreview } from '@/lib/backend';
import type { PasswordEntry,Tag } from '@/types';
import { createContext,useContext,useMemo } from 'react';


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


/** Keep only list metadata; never retain decrypted fields or staged attachment bytes. */
export function passwordEntryToPreview(entry: PasswordEntry): PasswordEntry {
  return {
    id: entry.id, title: entry.title, username: entry.username, password: '••••••••',
    url: entry.url, email: entry.email, notes: '', tags: [...entry.tags],
    favorite: entry.favorite, pinned: entry.pinned,
    totpSecret: entry.totpSecret ? 'has-totp' : undefined, customFields: [],
    createdAt: entry.createdAt, updatedAt: entry.updatedAt, breachStatus: entry.breachStatus,
    hasPasskey: entry.hasPasskey, attachmentCount: entry.attachmentCount ?? entry.attachments?.length ?? 0,
  };
}


export function needsBreachRecheck(status: BreachStatus | undefined, now = Date.now()): boolean {
  if (!status || !('checked_at' in status)) return true;
  const checkedAt = Date.parse(status.checked_at);
  return !Number.isFinite(checkedAt) || checkedAt > now || now - checkedAt >= 24 * 60 * 60 * 1000;
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
      type: f.field_type.toLowerCase() as PasswordEntry['customFields'][number]['type'],
      value: f.value,
      sensitive: f.sensitive,
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
  ) => Promise<boolean>;
  bulkDeleteEntries: (ids: string[]) => Promise<boolean>;
  updateEntry: (entry: PasswordEntry) => Promise<boolean>;
  updateBreachStatus: (id: string, breachStatus: BreachStatus) => Promise<void>;
  saveVault: () => Promise<void>;
  addEntry: (entry: PasswordEntry) => Promise<boolean>;
  deleteEntry: (id: string) => Promise<boolean>;
  deleteAttachment: (entryId: string, attachmentId: string) => Promise<void>;
  toggleFavorite: (id: string) => Promise<void>;
  togglePin: (id: string) => Promise<void>;
  selectEntryById: (id: string | null) => Promise<void>;
  addTag: (tag: Tag) => Promise<boolean>;
  updateTag: (id: string, updates: Partial<Tag>) => Promise<boolean>;
  removeTag: (id: string) => Promise<boolean>;
  reorderTags: (newTags: Tag[]) => Promise<void>;
  refreshEntries: () => Promise<void>;
  refreshTags: () => Promise<void>;
  isP2pListening: boolean;
  p2pReceiverEnabled: boolean;
  toggleP2pListener: (enable?: boolean) => void;
  localLanIp: string | null;
}

export const EntriesContext = createContext<EntriesContextType | undefined>(undefined);


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
