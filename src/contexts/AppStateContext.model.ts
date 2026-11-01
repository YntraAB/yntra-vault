import type { BreachStatus } from '@/lib/backend';
import type { AppSettings,FilterCategory,PasswordEntry,Tag,ToastMessage,Vault } from '@/types';

import { useAuth } from '@/features/auth';
import { useEntries,useFilteredEntries } from '@/features/entries';
import { useSettings } from '@/features/settings';
import { useToast } from './ToastContext';
import { useSearch,useUi } from './UiContext';


// ─── Re-export Granular Hooks and Types ─────────────────────────────────

export { SettingsProvider,useSettings } from '@/features/settings';

export { ToastProvider,useToast } from './ToastContext';

export { AuthProvider,useAuth } from '@/features/auth';

export { UiProvider,useSearch,useUi } from './UiContext';

export { EntriesProvider,useEntries,useFilteredEntries } from '@/features/entries';


export interface AppStateContextType {
  entries: PasswordEntry[];
  filteredEntries: PasswordEntry[];
  tags: Tag[];
  selectedEntry: PasswordEntry | null;
  selectedEntryIds: string[];
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
  isEntryModalOpen: boolean;
  setIsEntryModalOpen: (open: boolean) => void;
  editingEntry?: PasswordEntry | null;
  openNewEntryModal?: () => void;
  openEditModal?: (entry: PasswordEntry) => void;
  closeEntryModal?: () => void;
  setSelectedEntry: (entry: PasswordEntry | null) => void;
  setSearchTerm: (term: string) => void;
  setFilterCategory: (cat: FilterCategory) => void;
  setIsEditing: (editing: boolean) => void;
  setSettingsOpen: (open: boolean) => void;
  updateEntry: (entry: PasswordEntry) => Promise<boolean>;
  updateBreachStatus: (id: string, breachStatus: BreachStatus) => Promise<void>;
  saveVault: () => Promise<void>;
  addEntry: (entry: PasswordEntry) => Promise<boolean>;
  deleteEntry: (id: string) => Promise<boolean>;
  deleteAttachment: (entryId: string, attachmentId: string) => Promise<void>;
  updateSettings: (partial: Partial<AppSettings>) => void;
  addToast: (msg: Omit<ToastMessage, 'id'>) => void;
  removeToast: (id: string) => void;
  setCurrentVault: (vault: Vault | null) => void;
  setIsLocked: (locked: boolean) => void;
  lockVault: () => Promise<void>;
  toggleFavorite: (id: string) => Promise<void>;
  togglePin: (id: string) => Promise<void>;
  selectEntryById: (id: string | null) => Promise<void>;
  addTag: (tag: Tag) => Promise<boolean>;
  updateTag: (id: string, updates: Partial<Tag>) => Promise<boolean>;
  removeTag: (id: string) => Promise<boolean>;
  addVault: (vault: Vault) => void;
  removeVault: (id: string) => void;
  refreshEntries: () => Promise<void>;
  refreshTags: () => Promise<void>;
}


// ─── Backward-compatible Facade Hook ────────────────────────────────────

export function useAppState(): AppStateContextType {
  const auth = useAuth();
  const settingsCtx = useSettings();
  const toastsCtx = useToast();
  const ui = useUi();
  const search = useSearch();
  const entries = useEntries();
  const filteredEntries = useFilteredEntries();

  return {
    ...entries,
    filteredEntries,
    ...auth,
    ...settingsCtx,
    ...toastsCtx,
    ...ui,
    ...search,
  };
}
