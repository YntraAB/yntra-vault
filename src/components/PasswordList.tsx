import { Search, X, Plus, Star, Pin, ShieldAlert } from 'lucide-react';
import { motion, AnimatePresence } from 'framer-motion';
import { useAppState } from '@/contexts/AppStateContext';
import EntryModal from './EntryModal';
import Favicon from './Favicon';
import type { PasswordEntry, Tag } from '@/types';
import { isToday, isYesterday } from '@/lib/utils';
import { useState, useMemo } from 'react';
import { Skeleton } from './ui/skeleton';

interface PasswordListProps {
  onResizeStart: (e: React.MouseEvent) => void;
}

interface Section {
  title: string;
  items: PasswordEntry[];
}

export default function PasswordList({ onResizeStart }: PasswordListProps) {
  const {
    filteredEntries,
    selectedEntry,
    searchTerm,
    setSearchTerm,
    filterCategory,
    tags,
    setIsEditing,
    selectEntryById,
    isLoadingEntries,
    settings,
  } = useAppState();

  const [showEntryModal, setShowEntryModal] = useState(false);

  const sections = useMemo<Section[]>(() => {
    const pinned = filteredEntries.filter((e) => e.pinned);
    const today = filteredEntries.filter((e) => !e.pinned && isToday(e.updatedAt));
    const yesterday = filteredEntries.filter((e) => !e.pinned && isYesterday(e.updatedAt));
    const earlier = filteredEntries.filter((e) => !e.pinned && !isToday(e.updatedAt) && !isYesterday(e.updatedAt));

    const result: Section[] = [];
    if (pinned.length) result.push({ title: 'Pinned', items: pinned });
    if (today.length) result.push({ title: 'Today', items: today });
    if (yesterday.length) result.push({ title: 'Yesterday', items: yesterday });
    if (earlier.length) result.push({ title: 'Earlier', items: earlier });
    return result;
  }, [filteredEntries]);

  const headerTitle = useMemo(() => {
    if (filterCategory === 'all') return 'All Items';
    if (filterCategory === 'favorites') return 'Favorites';
    return filterCategory;
  }, [filterCategory]);

  return (
    <div
      className="relative flex h-full flex-col border-r border-[var(--border-subtle)] bg-[var(--bg-surface)]"
      style={{ width: 'var(--passwordlist-width)' }}
    >
      {/* Header */}
      <div className="flex flex-col gap-3 p-3 pb-2">
        {/* Title row */}
        <div className="flex items-center justify-between">
          <h2 className="text-[16px] font-semibold leading-tight tracking-tight text-[var(--text-primary)]">
            {headerTitle}
          </h2>
          <span className="text-[12px] tabular-nums text-[var(--text-tertiary)]">
            {filteredEntries.length}
          </span>
        </div>

        {/* Search */}
        <div className="flex h-8 items-center gap-1.5 rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] px-2">
          <Search size={14} className="shrink-0 text-[var(--text-tertiary)]" />
          <input
            type="text"
            placeholder="Search..."
            value={searchTerm}
            onChange={(e) => setSearchTerm(e.target.value)}
            className="flex-1 bg-transparent text-[13px] text-[var(--text-primary)] outline-none placeholder:text-[var(--text-tertiary)]"
          />
          {searchTerm && (
            <button
              onClick={() => setSearchTerm('')}
              className="inline-flex shrink-0 items-center justify-center rounded-[3px] p-0.5 text-[var(--text-tertiary)] hover:text-[var(--text-primary)]"
            >
              <X size={14} />
            </button>
          )}
        </div>

        {/* Add button */}
        <button
          onClick={() => setShowEntryModal(true)}
          className="flex h-8 items-center justify-center gap-1.5 rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] text-[13px] font-medium text-[var(--text-secondary)] transition-colors hover:border-[var(--border-focus)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"
        >
          <Plus size={14} />
          Add Password
        </button>
      </div>

      {/* List */}
      <div className="flex-1 overflow-y-auto">
        <AnimatePresence mode="wait">
          {isLoadingEntries ? (
            <motion.div
              key="loading-list"
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              transition={{ duration: 0.1, ease: 'easeInOut' }}
              className="flex flex-col"
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
              className="flex flex-col items-center justify-center py-16"
            >
              <p className="text-[13px] text-[var(--text-tertiary)]">No entries found</p>
            </motion.div>
          ) : (
            <motion.div
              key="populated-list"
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              transition={{ duration: 0.1, ease: 'easeInOut' }}
              className="flex flex-col"
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
                      selected={selectedEntry?.id === entry.id}
                      tags={tags}
                      showBreach={settings.showBreachInList}
                      onClick={() => {
                        if (selectedEntry?.id !== entry.id) {
                          selectEntryById(entry.id);
                          setIsEditing(false);
                        }
                      }}
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
        aria-label="Resize password list"
      />

      <EntryModal
        open={showEntryModal}
        onClose={() => setShowEntryModal(false)}
      />
    </div>
  );
}

function ListItem({
  entry,
  selected,
  tags,
  showBreach,
  onClick,
}: {
  entry: PasswordEntry;
  selected: boolean;
  tags: Tag[];
  showBreach: boolean;
  onClick: () => void;
}) {
  const tagColors = entry.tags
    .map((t) => tags.find((tag) => tag.name === t)?.color)
    .filter(Boolean) as string[];

  return (
    <motion.button
      initial={{ opacity: 0 }}
      animate={{ opacity: 1 }}
      transition={{ duration: 0.1 }}
      onClick={onClick}
      className={`group flex h-12 w-full items-center gap-3 border-b border-[var(--border-subtle)] px-3 text-left transition-colors ${
        selected
          ? 'border-l-2 border-l-[var(--text-primary)] bg-[var(--bg-active)]'
          : 'border-l-2 border-l-transparent hover:bg-[var(--bg-hover)]'
      }`}
    >
      {/* Favicon */}
      <Favicon
        url={entry.url}
        title={entry.title}
        color={tagColors[0]}
        sizeClass="h-7 w-7"
        textClass="text-[11px]"
      />

      {/* Text */}
      <div className="flex min-w-0 flex-1 flex-col">
        <span className="truncate text-[14px] font-medium leading-tight text-[var(--text-primary)]">
          {entry.title}
        </span>
        {(() => {
          if (entry.username) {
            return (
              <span className="truncate text-[12px] leading-tight text-[var(--text-secondary)]">
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
                <span className="flex min-w-0 text-[12px] leading-tight text-[var(--text-secondary)]">
                  <span className="truncate">{local}</span>
                  <span className="shrink-0">{domain}</span>
                </span>
              );
            }
            return (
              <span className="truncate text-[12px] leading-tight text-[var(--text-secondary)]">
                {text}
              </span>
            );
          }
          return (
            <span className="truncate text-[12px] leading-tight text-[var(--text-secondary)]">
              {''}
            </span>
          );
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
          <span title="Leaked in data breach">
            <ShieldAlert size={12} className="text-red-500 shrink-0 animate-pulse" />
          </span>
        )}

        {/* Pin indicator */}
        {entry.pinned && (
          <Pin size={11} className="text-yellow-500 fill-current" />
        )}

        {/* Favorite star */}
        {entry.favorite && (
          <Star size={11} className="text-orange-500 fill-current" />
        )}
      </div>
    </motion.button>
  );
}



