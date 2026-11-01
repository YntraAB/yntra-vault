import { useAuth } from '@/features/auth';
import type { FilterCategory,PasswordEntry } from '@/types';
import React,{ useCallback,useMemo,useState } from 'react';
import { SearchContext,UiContext } from './UiContext.model';


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

  const [wasLocked, setWasLocked] = useState(isLocked);
  if (isLocked !== wasLocked) {
    setWasLocked(isLocked);
    if (isLocked) {
      setSearchTerm(''); setIsEditing(false); setSettingsOpen(false);
      setIsEntryModalOpen(false); setEditingEntry(null); setIsCreateTagOpen(false);
    }
  }

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