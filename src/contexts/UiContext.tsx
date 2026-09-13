import React, { createContext, useContext, useState, useEffect, useMemo } from 'react';
import type { FilterCategory } from '@/types';
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
  const [isCreateTagOpen, setIsCreateTagOpen] = useState(false);
  const { isLocked } = useAuth();

  // Reset UI and search states on lock
  useEffect(() => {
    if (isLocked) {
      setSearchTerm('');
      setIsEditing(false);
      setSettingsOpen(false);
      setIsEntryModalOpen(false);
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
      isCreateTagOpen,
      setIsCreateTagOpen,
    }),
    [filterCategory, isEditing, settingsOpen, isEntryModalOpen, isCreateTagOpen]
  );

  return (
    <SearchContext.Provider value={searchValue}>
      <UiContext.Provider value={uiValue}>
        {children}
      </UiContext.Provider>
    </SearchContext.Provider>
  );
}
