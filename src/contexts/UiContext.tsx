import React, { createContext, useContext, useState, useEffect, useMemo, useCallback } from 'react';
import type { FilterCategory, PasswordEntry } from '@/types';
import { useAuth } from '@/features/auth';

// ─── Search Context (High-Frequency) ────────────────────────────────────

export interface SearchContextType {
  searchTerm: string;
  setSearchTerm: (term: string) => void;
}

const SearchContext = createContext<SearchContextType | undefined>(undefined);

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

const UiContext = createContext<UiContextType | undefined>(undefined);

export function useUi(): UiContextType {
  const ctx = useContext(UiContext);
  if (!ctx) throw new Error('useUi must be used within UiProvider');
  return ctx;
}

// ─── Provider Composing Both Contexts ───────────────────────────────────

export function UiProvider({ children }: { children: React.ReactNode }) {
  const [searchTerm, setSearchTerm] = useState('');
  const [filterCategory, setFilterCategory] = useState<FilterCategory>('all');
  const [isEditing, setIsEditing] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [isEntryModalOpen, setIsEntryModalOpen] = useState(false);
  const [editingEntry, setEditingEntry] = useState<PasswordEntry | null>(null);
  const [isCreateTagOpen, setIsCreateTagOpen] = useState(false);
  const { isLocked } = useAuth();

  const openNewEntryModal = useCallback(() => {
    setIsEditing(false);
    setEditingEntry(null);
    setIsEntryModalOpen(true);
  }, []);

  const openEditModal = useCallback((entry: PasswordEntry) => {
    setIsEditing(false);
    setEditingEntry(entry);
    setIsEntryModalOpen(true);
  }, []);

  const closeEntryModal = useCallback(() => {
    setIsEntryModalOpen(false);
    setEditingEntry(null);
  }, []);

  // Reset UI and search states on lock
  useEffect(() => {
    if (isLocked) {
      setSearchTerm('');
      setIsEditing(false);
      setSettingsOpen(false);
      setIsEntryModalOpen(false);
      setEditingEntry(null);
      setIsCreateTagOpen(false);
    }
  }, [isLocked]);

  const searchValue = useMemo(() => ({ searchTerm, setSearchTerm }), [searchTerm]);
  const uiValue = useMemo(
    () => ({
      filterCategory,
      setFilterCategory,
      isEditing,
      setIsEditing,
      settingsOpen,
      setSettingsOpen,
      isEntryModalOpen,
      setIsEntryModalOpen,
      editingEntry,
      setEditingEntry,
      openNewEntryModal,
      openEditModal,
      closeEntryModal,
      isCreateTagOpen,
      setIsCreateTagOpen,
    }),
    [
      filterCategory,
      isEditing,
      settingsOpen,
      isEntryModalOpen,
      editingEntry,
      openNewEntryModal,
      openEditModal,
      closeEntryModal,
      isCreateTagOpen,
    ]
  );

  return (
    <SearchContext.Provider value={searchValue}>
      <UiContext.Provider value={uiValue}>
        {children}
      </UiContext.Provider>
    </SearchContext.Provider>
  );
}
