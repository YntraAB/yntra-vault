import type { FilterCategory,PasswordEntry } from '@/types';
import { createContext,useContext } from 'react';


// ─── Search Context (High-Frequency) ────────────────────────────────────

export interface SearchContextType {
  searchTerm: string;
  setSearchTerm: (term: string) => void;
}

export const SearchContext = createContext<SearchContextType | undefined>(undefined);


export function useSearch(): SearchContextType {
  const ctx = useContext(SearchContext);
  if (!ctx) throw new Error('useSearch must be used within UiProvider');
  return ctx;
}


// ─── UI Modals & Filters Context ────────────────────────────────────────

export interface UiContextType {
  filterCategory: FilterCategory;
  setFilterCategory: (cat: FilterCategory) => void;
  isEditing: boolean;
  setIsEditing: (editing: boolean) => void;
  settingsOpen: boolean;
  setSettingsOpen: (open: boolean) => void;
  isEntryModalOpen: boolean;
  setIsEntryModalOpen: (open: boolean) => void;
  editingEntry: PasswordEntry | null;
  setEditingEntry: (entry: PasswordEntry | null) => void;
  openNewEntryModal: () => void;
  openEditModal: (entry: PasswordEntry) => void;
  closeEntryModal: () => void;
  isCreateTagOpen: boolean;
  setIsCreateTagOpen: (open: boolean) => void;
}

export const UiContext = createContext<UiContextType | undefined>(undefined);


export function useUi(): UiContextType {
  const ctx = useContext(UiContext);
  if (!ctx) throw new Error('useUi must be used within UiProvider');
  return ctx;
}