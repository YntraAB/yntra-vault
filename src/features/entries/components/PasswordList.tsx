import { useState, useMemo, useCallback, useEffect, useRef } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import {
  Search,
  X,
  Plus,
  Star,
  Pin,
  ShieldAlert,
  Paperclip,
} from 'lucide-react';
import { useFilteredEntries, useEntries } from '../context/EntriesContext';
import { useSearch, useUi } from '@/contexts/UiContext';
import { useSettings } from '@/features/settings';
import { useToast } from '@/contexts/ToastContext';
import { useTranslation } from '@/contexts/LanguageContext';
import { useBackend } from '@/lib/useBackend';
import { BulkEditModal } from './BulkEditModal';
import { EntryContextMenu } from './EntryContextMenu';
import { PasswordListAreaContextMenu } from './PasswordListAreaContextMenu';
import { DeleteEntryModal } from './DeleteEntryModal';
import { Favicon } from './Favicon';
import type { PasswordEntry, Tag } from '@/types';
import { isToday, isYesterday } from '@/lib/utils';
import { Skeleton } from '@/components/ui';
import { ActionTooltip } from '@/components/ui/tooltip';
import { matchesShortcut, getKeybinds } from '@/lib/keybinds';

export interface PasswordListProps {
  onResizeStart?: (e: React.MouseEvent) => void;
}

interface Section {
  title: string;
  items: PasswordEntry[];
}

export function PasswordList({ onResizeStart }: PasswordListProps) {
  const { t } = useTranslation();
  const { backend } = useBackend();
  const filteredEntries = useFilteredEntries();
  const { searchTerm, setSearchTerm } = useSearch();
  const { filterCategory, setIsEditing, settingsOpen, isEntryModalOpen, openNewEntryModal, openEditModal } = useUi();
  const {
    selectedEntry,
    tags,
    selectEntryById,
    isLoadingEntries,
    deleteEntry,
    toggleFavorite,
    togglePin,
    selectedEntryIds,
    toggleEntrySelection,
    bulkDeleteEntries,
  } = useEntries();
  const { settings, updateSettings } = useSettings();
  const { addToast } = useToast();

  const [showBulkEditModal, setShowBulkEditModal] = useState(false);

  const [contextMenu, setContextMenu] = useState<{
    open: boolean;
    x: number;
    y: number;
    entry: PasswordEntry | null;
  }>({ open: false, x: 0, y: 0, entry: null });

  // Area context menu (right click empty space)
  const [areaContextMenu, setAreaContextMenu] = useState<{
    open: boolean;
    x: number;
    y: number;
  }>({ open: false, x: 0, y: 0 });

  const handleListAreaContextMenu = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    setAreaContextMenu({ open: true, x: e.clientX, y: e.clientY });
  }, []);

  const [deleteConfirmEntry, setDeleteConfirmEntry] = useState<PasswordEntry | null>(null);

  const deleteBtnRef = useRef<HTMLButtonElement>(null);
  const cancelBtnRef = useRef<HTMLButtonElement>(null);
  const searchInputRef = useRef<HTMLInputElement>(null);

  // Focus search input on configured keybind (default: Ctrl+K) unless in a dialog or settings
  useEffect(() => {
    const handleGlobalKeyDown = (e: KeyboardEvent) => {
      const kb = getKeybinds(settings.keybinds);

      if (matchesShortcut(e, kb.search)) {
        if (settingsOpen || isEntryModalOpen || deleteConfirmEntry) {
          return;
        }

        const hasOpenDialog = Boolean(
          document.querySelector('[role="dialog"], [aria-modal="true"], dialog[open], .fixed.inset-0')
        );

        if (hasOpenDialog) {
          return;
        }

        e.preventDefault();
        e.stopPropagation();
        if (searchInputRef.current) {
          searchInputRef.current.focus();
          searchInputRef.current.select();
        }
      }
    };

    window.addEventListener('keydown', handleGlobalKeyDown, true);
    return () => window.removeEventListener('keydown', handleGlobalKeyDown, true);
  }, [settings.keybinds, settingsOpen, isEntryModalOpen, deleteConfirmEntry]);

  // Listen for Ctrl+E / Cmd+E for bulk edit when multiple items are selected
  useEffect(() => {
    const handleBulkEditShortcut = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'e') {
        if (selectedEntryIds.length >= 2) {
          e.preventDefault();
          setShowBulkEditModal(true);
        }
      }
    };
    window.addEventListener('keydown', handleBulkEditShortcut);
    return () => window.removeEventListener('keydown', handleBulkEditShortcut);
  }, [selectedEntryIds]);

  // Auto-focus and focus trap for delete confirmation dialog
  useEffect(() => {
    if (deleteConfirmEntry) {
      const timer = setTimeout(() => deleteBtnRef.current?.focus(), 50);

      const handleKeyDown = (e: KeyboardEvent) => {
        if (e.key === 'Escape') {
          e.preventDefault();
          e.stopPropagation();
          setDeleteConfirmEntry(null);
        } else if (e.key === 'Tab') {
          e.preventDefault();
          if (document.activeElement === deleteBtnRef.current) {
            cancelBtnRef.current?.focus();
          } else {
            deleteBtnRef.current?.focus();
          }
        }
      };

      window.addEventListener('keydown', handleKeyDown, true);
      return () => {
        clearTimeout(timer);
        window.removeEventListener('keydown', handleKeyDown, true);
      };
    }
  }, [deleteConfirmEntry]);

  const handleEntryContextMenu = useCallback((e: React.MouseEvent, entry: PasswordEntry) => {
    e.preventDefault();
    e.stopPropagation();
    if (!selectedEntryIds.includes(entry.id)) {
      toggleEntrySelection(entry.id, false, false);
    }
    setContextMenu({ open: true, x: e.clientX, y: e.clientY, entry });
  }, [selectedEntryIds, toggleEntrySelection]);

  const handleAddEntryClick = useCallback(() => {
    openNewEntryModal();
  }, [openNewEntryModal]);

  const handleRename = useCallback((entry: PasswordEntry) => {
    selectEntryById(entry.id);
    openEditModal(entry);
  }, [selectEntryById, openEditModal]);

  const handleDelete = useCallback((entry: PasswordEntry) => {
    setDeleteConfirmEntry(entry);
  }, []);

  const confirmDeleteEntry = useCallback(() => {
    if (deleteConfirmEntry) {
      deleteEntry(deleteConfirmEntry.id);
      addToast({ message: t('toast.entry_deleted', { title: deleteConfirmEntry.title }), type: 'info' });
      setDeleteConfirmEntry(null);
    }
  }, [deleteConfirmEntry, deleteEntry, addToast, t]);

  const handleAutotype = useCallback(async (entry: PasswordEntry) => {
    if (!backend) return;
    addToast({ message: t('toast.autotype_starting', { title: entry.title }), type: 'info' });
    try {
      await backend.runSmartAutotype(
        entry.username || '',
        entry.password || '',
        entry.totpSecret || '',
        entry.url || '',
        settings.autotypeLaunchBrowser !== false,
        settings.autotypeCharDelayMs || 15,
        settings.autotypeFieldDelayMs || 300
      );
    } catch (err) {
      addToast({ message: t('toast.autotype_failed', { err: String(err) }), type: 'error' });
    }
  }, [backend, settings, addToast, t]);

  const handleToggleFavorite = useCallback((entry: PasswordEntry) => {
    toggleFavorite(entry.id);
  }, [toggleFavorite]);

  const handleTogglePin = useCallback((entry: PasswordEntry) => {
    togglePin(entry.id);
  }, [togglePin]);

  const sections = useMemo<Section[]>(() => {
    const pinned = filteredEntries.filter((e) => e.pinned);
    const today = filteredEntries.filter((e) => !e.pinned && isToday(e.updatedAt));
    const yesterday = filteredEntries.filter((e) => !e.pinned && isYesterday(e.updatedAt));
    const earlier = filteredEntries.filter((e) => !e.pinned && !isToday(e.updatedAt) && !isYesterday(e.updatedAt));

    const result: Section[] = [];
    if (pinned.length) result.push({ title: t('detail.pin'), items: pinned });
    if (today.length) result.push({ title: t('time.today'), items: today });
    if (yesterday.length) result.push({ title: t('time.yesterday'), items: yesterday });
    if (earlier.length) result.push({ title: t('time.earlier'), items: earlier });
    return result;
  }, [filteredEntries, t]);

  const headerTitle = useMemo(() => {
    if (filterCategory === 'all') return t('sidebar.all_items');
    if (filterCategory === 'favorites') return t('sidebar.favorites');
    return filterCategory;
  }, [filterCategory, t]);

  const handleItemClick = useCallback(
    (e: React.MouseEvent, entry: PasswordEntry) => {
      const isMulti = e.ctrlKey || e.metaKey;
      const isRange = e.shiftKey;
      if (isMulti || isRange) {
        toggleEntrySelection(entry.id, isMulti, isRange, filteredEntries);
      } else {
        selectEntryById(entry.id);
        toggleEntrySelection(entry.id, false, false);
        setIsEditing(false);
      }
    },
    [toggleEntrySelection, selectEntryById, setIsEditing, filteredEntries]
  );

  return (
    <div
      className="relative flex h-full flex-col border-r border-[var(--border-subtle)] bg-[var(--bg-surface)] select-none"
      style={{ width: 'var(--passwordlist-width)' }}
    >
      {/* Desktop Header */}
      <div className="hidden md:flex h-12 shrink-0 items-center justify-between border-b border-[var(--border-subtle)] px-3">
        <h1 className="text-[14px] font-semibold text-[var(--text-primary)]">
          {headerTitle}
        </h1>
        <span className="text-[12px] font-medium text-[var(--text-tertiary)]">
          {selectedEntryIds.length > 1
            ? `${selectedEntryIds.length} selected`
            : t('list.items_count', { count: filteredEntries.length })}
        </span>
      </div>

      {/* Desktop Toolbar */}
      <div className="hidden md:flex shrink-0 flex-col gap-2 p-2">
        {/* Search */}
        <div className="flex h-8 items-center gap-2 rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] px-2.5 transition-colors focus-within:border-[var(--border-focus)]">
          <Search size={14} className="shrink-0 text-[var(--text-tertiary)]" />
          <input
            id="search-input"
            ref={searchInputRef}
            type="text"
            placeholder={t('app.search_placeholder')}
            value={searchTerm}
            onChange={(e) => setSearchTerm(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Escape') {
                if (searchTerm) {
                  e.preventDefault();
                  e.stopPropagation();
                  setSearchTerm('');
                } else {
                  searchInputRef.current?.blur();
                }
              }
            }}
            className="flex-1 bg-transparent text-[13px] text-[var(--text-primary)] outline-none placeholder:text-[var(--text-tertiary)]"
          />
          {searchTerm && (
            <ActionTooltip content={t('list.clear_search')}>
              <button
                onClick={() => setSearchTerm('')}
                className="inline-flex shrink-0 items-center justify-center rounded-[3px] p-0.5 text-[var(--text-tertiary)] hover:text-[var(--text-primary)]"
              >
                <X size={14} />
              </button>
            </ActionTooltip>
          )}
        </div>

        {/* Add button / Bulk edit bar */}
        {selectedEntryIds.length >= 2 ? (
          <button
            onClick={() => setShowBulkEditModal(true)}
            className="flex h-8 items-center justify-center gap-1.5 rounded-[3px] border border-[var(--accent-primary)]/40 bg-[var(--accent-primary)]/10 text-[13px] font-medium text-[var(--accent-primary)] transition-colors hover:bg-[var(--accent-primary)]/20"
          >
            Edit {selectedEntryIds.length} Selected Entries (Ctrl+E)
          </button>
        ) : (
          <button
            onClick={handleAddEntryClick}
            className="flex h-8 items-center justify-center gap-1.5 rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] text-[13px] font-medium text-[var(--text-secondary)] transition-colors hover:border-[var(--border-focus)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"
          >
            <Plus size={14} />
            {t('list.new_entry')}
          </button>
        )}
      </div>

      {/* List */}
      <div
        className="flex flex-1 flex-col overflow-y-auto pb-[calc(4rem+env(safe-area-inset-bottom,0px))] md:pb-0"
        onContextMenu={handleListAreaContextMenu}
      >
        <AnimatePresence mode="wait">
          {isLoadingEntries ? (
            <motion.div
              key="loading-list"
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              transition={{ duration: 0.1, ease: 'easeInOut' }}
              className="flex flex-1 flex-col"
            >
              {[...Array(6)].map((_, i) => (
                <div key={i} className="flex h-12 w-full items-center gap-3 border-b border-[var(--border-subtle)] px-3">
                  <Skeleton className="h-7 w-7 rounded-full shrink-0" />
                  <div className="flex-1 min-w-0">
                    <Skeleton className="h-4 w-32 rounded" />
                  </div>
                </div>
              ))}
            </motion.div>
          ) : filteredEntries.length === 0 ? (
            <motion.div
              key="empty-list"
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              transition={{ duration: 0.1, ease: 'easeInOut' }}
              className="flex flex-1 flex-col items-center justify-center py-16"
            >
              <p className="text-[13px] text-[var(--text-tertiary)]">{t('list.empty_title')}</p>
            </motion.div>
          ) : (
            <motion.div
              key="populated-list"
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              transition={{ duration: 0.1, ease: 'easeInOut' }}
              className="flex flex-1 flex-col min-h-full"
            >
              {sections.map((section) => (
                <div key={section.title}>
                  <div className="sticky top-0 z-10 flex h-7 items-center bg-[var(--bg-surface)] px-3">
                    <span className="text-[11px] font-semibold uppercase tracking-[0.04em] text-[var(--text-tertiary)]">
                      {section.title}
                    </span>
                  </div>
                  {section.items.map((entry) => (
                    <ListItem
                      key={entry.id}
                      entry={entry}
                      selected={selectedEntryIds.includes(entry.id) || selectedEntry?.id === entry.id}
                      tags={tags}
                      showBreach={settings.showBreachInList}
                      density={settings.density}
                      onClick={(e) => handleItemClick(e, entry)}
                      onContextMenu={(e) => handleEntryContextMenu(e, entry)}
                    />
                  ))}
                </div>
              ))}
            </motion.div>
          )}
        </AnimatePresence>
      </div>

      {/* Resize handle */}
      <div
        className="absolute right-0 top-0 z-10 h-full w-[3px] cursor-col-resize transition-colors hover:bg-[var(--border-focus)]"
        onMouseDown={onResizeStart}
        role="slider"
        aria-label={t('common.resize_password_list')}
      />

      <BulkEditModal
        open={showBulkEditModal}
        selectedIds={selectedEntryIds}
        onClose={() => setShowBulkEditModal(false)}
      />

      {/* Entry Context Menu */}
      <EntryContextMenu
        open={contextMenu.open}
        x={contextMenu.x}
        y={contextMenu.y}
        entry={contextMenu.entry}
        selectedCount={selectedEntryIds.length}
        onClose={() => setContextMenu((prev) => ({ ...prev, open: false }))}
        onRename={handleRename}
        onDelete={handleDelete}
        onAutotype={handleAutotype}
        onToggleFavorite={handleToggleFavorite}
        onTogglePin={handleTogglePin}
        onBulkEdit={() => setShowBulkEditModal(true)}
        onBulkDelete={() => bulkDeleteEntries(selectedEntryIds)}
      />

      {/* Password List Area Context Menu (Empty Background) */}
      <PasswordListAreaContextMenu
        open={areaContextMenu.open}
        x={areaContextMenu.x}
        y={areaContextMenu.y}
        onClose={() => setAreaContextMenu((prev) => ({ ...prev, open: false }))}
        onNewEntry={handleAddEntryClick}
        searchTerm={searchTerm}
        onClearSearch={() => setSearchTerm('')}
        sortOrder={settings.entrySortOrder ?? 'updated'}
        onSetSortOrder={(entrySortOrder) => updateSettings({ entrySortOrder })}
      />

      {/* Delete Entry Confirmation Overlay */}
      <DeleteEntryModal
        entry={deleteConfirmEntry}
        onClose={() => setDeleteConfirmEntry(null)}
        onConfirm={confirmDeleteEntry}
      />
    </div>
  );
}

function ListItem({
  entry,
  selected,
  tags,
  showBreach,
  density = 'normal',
  onClick,
  onContextMenu,
}: {
  entry: PasswordEntry;
  selected: boolean;
  tags: Tag[];
  showBreach: boolean;
  density?: 'compact' | 'normal' | 'comfortable';
  onClick: (e: React.MouseEvent) => void;
  onContextMenu: (e: React.MouseEvent) => void;
}) {
  const { t } = useTranslation();
  const tagColors = entry.tags
    .map((t) => tags.find((tag) => tag.name === t)?.color)
    .filter(Boolean) as string[];

  let itemHeightClass = 'h-12 py-2';
  let faviconSizeClass = 'h-7 w-7';
  let titleTextClass = 'text-[14px]';
  let subTextClass = 'text-[12px]';

  if (density === 'compact') {
    itemHeightClass = 'h-10 py-1';
    faviconSizeClass = 'h-6 w-6';
    titleTextClass = 'text-[13px]';
    subTextClass = 'text-[11px]';
  } else if (density === 'comfortable') {
    itemHeightClass = 'h-14 py-2.5';
    faviconSizeClass = 'h-8 w-8';
    titleTextClass = 'text-[15px]';
    subTextClass = 'text-[13px]';
  }

  // Long press timer for mobile touch viewports
  const longPressTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const handleTouchStart = (e: React.TouchEvent) => {
    longPressTimerRef.current = setTimeout(() => {
      onContextMenu({
        preventDefault: () => {},
        stopPropagation: () => {},
        clientX: e.touches[0].clientX,
        clientY: e.touches[0].clientY,
      } as any);
    }, 500);
  };

  const handleTouchEnd = () => {
    if (longPressTimerRef.current) {
      clearTimeout(longPressTimerRef.current);
    }
  };

  return (
    <motion.button
      initial={{ opacity: 0 }}
      animate={{ opacity: 1 }}
      transition={{ duration: 0.1 }}
      onClick={onClick}
      onContextMenu={onContextMenu}
      onTouchStart={handleTouchStart}
      onTouchEnd={handleTouchEnd}
      onTouchMove={handleTouchEnd}
      className={`group flex ${itemHeightClass} w-full items-center gap-3 border-b border-[var(--border-subtle)] px-3.5 text-left transition-all active:bg-[var(--bg-hover)] ${
        selected
          ? 'border-l-3 border-l-[var(--text-primary)] bg-[var(--bg-active)]'
          : 'border-l-3 border-l-transparent hover:bg-[var(--bg-hover)]'
      }`}
    >
      {/* Favicon */}
      <Favicon
        url={entry.url}
        title={entry.title}
        color={tagColors[0]}
        sizeClass={faviconSizeClass}
        textClass="text-[11px]"
      />

      {/* Text */}
      <div className="flex min-w-0 flex-1 flex-col">
        <span className={`truncate ${titleTextClass} font-medium leading-tight text-[var(--text-primary)]`}>
          {entry.title}
        </span>
        {(() => {
          if (entry.username) {
            return (
              <span className={`truncate ${subTextClass} leading-tight text-[var(--text-secondary)] mt-0.5`}>
                {entry.username}
              </span>
            );
          }
          if (entry.email) {
            const text = entry.email;
            const atIndex = text.lastIndexOf('@');
            if (atIndex > 0) {
              const local = text.slice(0, atIndex);
              const domain = text.slice(atIndex);
              return (
                <span className={`flex min-w-0 ${subTextClass} leading-tight text-[var(--text-secondary)] mt-0.5`}>
                  <span className="truncate">{local}</span>
                  <span className="shrink-0">{domain}</span>
                </span>
              );
            }
            return (
              <span className={`truncate ${subTextClass} leading-tight text-[var(--text-secondary)] mt-0.5`}>
                {text}
              </span>
            );
          }
          return null;
        })()}
      </div>

      {/* Indicators */}
      <div className="flex items-center gap-1.5 shrink-0">
        {/* Tag dots */}
        {tagColors.length > 0 && (
          <div className="flex gap-1 mr-1">
            {tagColors.slice(0, 3).map((c, i) => (
              <span key={i} className="h-1.5 w-1.5 rounded-full" style={{ backgroundColor: c }} />
            ))}
          </div>
        )}

        {/* Breach Alert */}
        {showBreach && entry.breachStatus?.type === 'Breached' && (
          <ActionTooltip content={t('security.leaked_in_breach')}>
            <span>
              <ShieldAlert size={12} className="text-red-500 shrink-0 animate-pulse" />
            </span>
          </ActionTooltip>
        )}

        {/* Attachment indicator */}
        {((entry.attachmentCount || 0) > 0 || (entry.attachments && entry.attachments.length > 0)) && (
          <ActionTooltip content={t('entry.has_attachments')}>
            <span>
              <Paperclip size={11} className="text-[var(--text-tertiary)]" />
            </span>
          </ActionTooltip>
        )}

        {/* Pin indicator */}
        {entry.pinned && (
          <ActionTooltip content={t('entry.pinned_entry')}>
            <span>
              <Pin size={11} className="text-yellow-500 fill-current" />
            </span>
          </ActionTooltip>
        )}

        {/* Favorite star */}
        {entry.favorite && (
          <ActionTooltip content={t('entry.favorite_entry')}>
            <span>
              <Star size={11} className="text-orange-500 fill-current" />
            </span>
          </ActionTooltip>
        )}
      </div>
    </motion.button>
  );
}

export default PasswordList;
