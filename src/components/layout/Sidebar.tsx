import { useState, useCallback, useMemo, useRef, useEffect } from 'react';
import { Globe, Star, Plus, Settings, Lock } from 'lucide-react';
import { Reorder } from 'framer-motion';
import { useNavigate } from 'react-router-dom';
import { useAuth } from '@/features/auth';
import { useEntries, EditTagModal, DeleteTagModal, TagContextMenu, TagsAreaContextMenu } from '@/features/entries';
import { useSettings } from '@/features/settings';
import { useUi } from '@/contexts/UiContext';
import { useToast } from '@/contexts/ToastContext';
import { useTranslation } from '@/contexts/LanguageContext';
import type { Tag } from '@/types';
import { Skeleton } from '@/components/ui';
import { ActionTooltip } from '@/components/ui/tooltip';

export interface SidebarProps {
  onResizeStart: (e: React.MouseEvent) => void;
}

export function Sidebar({ onResizeStart }: SidebarProps) {
  const navigate = useNavigate();
  const { t } = useTranslation();
  const { tags, entries, removeTag, isLoadingEntries, reorderTags } = useEntries();
  const { filterCategory, setFilterCategory, settingsOpen, setSettingsOpen, setIsEntryModalOpen, setIsCreateTagOpen } = useUi();
  const { lockVault } = useAuth();
  const { settings, updateSettings } = useSettings();
  const { addToast } = useToast();

  const allCount = entries.length;
  const favCount = entries.filter((e) => e.favorite).length;

  const tagMap = useMemo(() => new Map(tags.map((t) => [t.id, t])), [tags]);
  const isCustomSort = (settings.tagSortOrder ?? 'custom') === 'custom';

  const sortedTags = useMemo(() => {
    const list = [...tags];
    const order = settings.tagSortOrder ?? 'custom';
    if (order === 'count') {
      return list.sort((a, b) => b.count - a.count || a.name.localeCompare(b.name));
    }
    if (order === 'name') {
      return list.sort((a, b) => a.name.localeCompare(b.name));
    }
    return list;
  }, [tags, settings.tagSortOrder]);

  const [orderedTagIds, setOrderedTagIds] = useState<string[]>(() => sortedTags.map((t) => t.id));
  const orderedTagIdsRef = useRef<string[]>(orderedTagIds);
  const initialDragOrderRef = useRef<string[]>([]);
  const isDraggingRef = useRef(false);
  const lastReorderTimeRef = useRef(0);
  const tagMapRef = useRef(tagMap);

  useEffect(() => {
    tagMapRef.current = tagMap;
  }, [tagMap]);

  // Synchronize tag IDs from sortedTags when not actively dragging and not recently reordered locally
  useEffect(() => {
    const next = sortedTags.map((t) => t.id);
    const current = orderedTagIdsRef.current;
    const tagCountChanged = current.length !== next.length;
    const tagIdsChanged = tagCountChanged || !next.every((id) => current.includes(id));

    // If tags were added, deleted, or vault changed, always synchronize immediately
    if (!tagIdsChanged) {
      if (isDraggingRef.current) return;
      if (Date.now() - lastReorderTimeRef.current < 2000) return;
    }

    const isSameOrder =
      !tagCountChanged &&
      current.every((id, idx) => id === next[idx]);
    if (!isSameOrder) {
      orderedTagIdsRef.current = next;
      setOrderedTagIds(next);
    }
  }, [sortedTags]);

  const displayTags = useMemo(() => {
    if (!isCustomSort) return sortedTags;
    return orderedTagIds
      .map((id) => tagMap.get(id))
      .filter((t): t is Tag => Boolean(t));
  }, [isCustomSort, sortedTags, orderedTagIds, tagMap]);

  const handleReorder = useCallback((newIds: string[]) => {
    lastReorderTimeRef.current = Date.now();
    orderedTagIdsRef.current = newIds;
    setOrderedTagIds(newIds);
  }, []);

  const handleDragEnd = useCallback(() => {
    lastReorderTimeRef.current = Date.now();
    const map = tagMapRef.current;
    const currentIds = orderedTagIdsRef.current;
    const initialIds = initialDragOrderRef.current;
    const hasOrderChanged =
      initialIds.length === currentIds.length &&
      initialIds.length > 0 &&
      initialIds.some((id, idx) => id !== currentIds[idx]);

    if (!hasOrderChanged) {
      return;
    }

    if (settings.tagSortOrder !== 'custom') {
      updateSettings({ tagSortOrder: 'custom' });
    }
    const newOrderTags = currentIds
      .map((id) => map.get(id))
      .filter((t): t is Tag => Boolean(t));
    if (newOrderTags.length > 0) {
      setTimeout(() => {
        reorderTags(newOrderTags);
      }, 50);
    }
  }, [settings.tagSortOrder, updateSettings, reorderTags]);

  const handleTagClick = useCallback((tagName: string) => {
    // Prevent accidental tag selection upon releasing a drag
    if (isDraggingRef.current || Date.now() - lastReorderTimeRef.current < 250) {
      return;
    }
    setFilterCategory(tagName);
  }, [setFilterCategory]);

  // Modal state
  const [editingTag, setEditingTag] = useState<Tag | null>(null);
  const [showEditTag, setShowEditTag] = useState(false);

  // Context menu state
  const [contextMenu, setContextMenu] = useState<{
    open: boolean;
    x: number;
    y: number;
    tag: Tag | null;
  }>({ open: false, x: 0, y: 0, tag: null });

  // Delete confirmation state
  const [deleteConfirmTag, setDeleteConfirmTag] = useState<Tag | null>(null);

  // Tags area context menu state
  const [areaContextMenu, setAreaContextMenu] = useState<{
    open: boolean;
    x: number;
    y: number;
  }>({ open: false, x: 0, y: 0 });

  const handleTagsAreaContextMenu = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    setAreaContextMenu({ open: true, x: e.clientX, y: e.clientY });
  }, []);

  const handleDeleteUnusedTags = useCallback(() => {
    const unused = tags.filter((t) => t.count === 0);
    if (unused.length === 0) return;
    unused.forEach((t) => removeTag(t.id));
    addToast({
      message: `Deleted ${unused.length} unused tag${unused.length > 1 ? 's' : ''}`,
      type: 'info',
    });
  }, [tags, removeTag, addToast]);

  const handleTagContextMenu = useCallback((e: React.MouseEvent, tag: Tag) => {
    e.preventDefault();
    e.stopPropagation();
    setContextMenu({ open: true, x: e.clientX, y: e.clientY, tag });
  }, []);

  const handleContextAddPassword = useCallback(() => {
    if (contextMenu.tag) {
      setFilterCategory(contextMenu.tag.name);
      setIsEntryModalOpen(true);
    }
  }, [contextMenu.tag, setFilterCategory, setIsEntryModalOpen]);

  const handleContextEdit = useCallback(() => {
    if (contextMenu.tag) {
      setEditingTag(contextMenu.tag);
      setShowEditTag(true);
    }
  }, [contextMenu.tag]);

  const handleContextDelete = useCallback(() => {
    if (contextMenu.tag) {
      setDeleteConfirmTag(contextMenu.tag);
    }
  }, [contextMenu.tag]);

  const confirmDelete = useCallback(() => {
    if (deleteConfirmTag) {
      if (filterCategory === deleteConfirmTag.name) {
        setFilterCategory('all');
      }
      removeTag(deleteConfirmTag.id);
      addToast({ message: t('toast.tag_deleted', { name: deleteConfirmTag.name }), type: 'info' });
      setDeleteConfirmTag(null);
    }
  }, [deleteConfirmTag, removeTag, addToast, filterCategory, setFilterCategory, t]);

  return (
    <aside
      className="relative flex h-full flex-col border-r border-[var(--border-subtle)] bg-[var(--bg-surface)] select-none"
      style={{ width: 'var(--sidebar-width)' }}
    >
      {/* Nav items */}
      <nav className="flex flex-col gap-[2px] p-2 mt-2">
        <NavItem
          icon={<Globe size={16} />}
          label={t('sidebar.all_items')}
          count={allCount}
          active={filterCategory === 'all'}
          density={settings.density}
          onClick={() => setFilterCategory('all')}
        />
        <NavItem
          icon={<Star size={16} />}
          label={t('sidebar.favorites')}
          count={favCount}
          active={filterCategory === 'favorites'}
          density={settings.density}
          onClick={() => setFilterCategory('favorites')}
        />
      </nav>

      {/* Tags */}
      <div
        className="mt-2 flex flex-1 flex-col overflow-hidden"
        onContextMenu={handleTagsAreaContextMenu}
      >
        <div className="flex h-7 items-center justify-between px-3">
          <span className="text-[11px] font-semibold uppercase tracking-[0.04em] text-[var(--text-tertiary)]">
            {t('sidebar.tags')}
          </span>
          <ActionTooltip content={t('sidebar.new_tag')} side="right">
            <button
              onClick={() => setIsCreateTagOpen(true)}
              className="inline-flex items-center justify-center rounded-[3px] p-1 text-[var(--text-tertiary)] transition-colors hover:text-[var(--text-primary)] cursor-pointer"
            >
              <Plus size={14} />
            </button>
          </ActionTooltip>
        </div>

        {isLoadingEntries ? (
          <div className="flex flex-col gap-[6px] px-2.5 py-1.5">
            {[...Array(4)].map((_, i) => (
              <div key={i} className="flex h-7 items-center gap-2">
                <Skeleton className="h-2.5 w-2.5 rounded-full shrink-0" />
                <Skeleton className="h-3.5 w-16" />
              </div>
            ))}
          </div>
        ) : isCustomSort ? (
          <Reorder.Group
            axis="y"
            values={orderedTagIds}
            onReorder={handleReorder}
            className="flex flex-1 flex-col gap-[2px] overflow-y-auto p-2 pt-1"
          >
            {displayTags.map((tag: Tag) => (
              <Reorder.Item
                key={tag.id}
                value={tag.id}
                onDragStart={() => {
                  initialDragOrderRef.current = [...orderedTagIdsRef.current];
                  isDraggingRef.current = true;
                  lastReorderTimeRef.current = Date.now();
                }}
                onDragEnd={() => {
                  isDraggingRef.current = false;
                  lastReorderTimeRef.current = Date.now();
                  handleDragEnd();
                }}
                className="relative select-none rounded-[3px] bg-[var(--bg-surface)] cursor-grab active:cursor-grabbing"
              >
                <TagItem
                  tag={tag}
                  active={filterCategory === tag.name}
                  density={settings.density}
                  showCount={settings.showTagCounts !== false}
                  onClick={() => handleTagClick(tag.name)}
                  onContextMenu={(e) => handleTagContextMenu(e, tag)}
                  onDoubleClick={() => {
                    setEditingTag(tag);
                    setShowEditTag(true);
                  }}
                />
              </Reorder.Item>
            ))}
          </Reorder.Group>
        ) : (
          <div className="flex flex-1 flex-col gap-[2px] overflow-y-auto p-2 pt-1">
            {displayTags.map((tag: Tag) => (
              <div key={tag.id} className="relative select-none">
                <TagItem
                  tag={tag}
                  active={filterCategory === tag.name}
                  density={settings.density}
                  showCount={settings.showTagCounts !== false}
                  onClick={() => handleTagClick(tag.name)}
                  onContextMenu={(e) => handleTagContextMenu(e, tag)}
                  onDoubleClick={() => {
                    setEditingTag(tag);
                    setShowEditTag(true);
                  }}
                />
              </div>
            ))}
          </div>
        )}
      </div>

      {/* Footer: Settings + Lock */}
      <div className="flex shrink-0 gap-1 border-t border-[var(--border-subtle)] p-2">
        <ActionTooltip content={t('sidebar.settings')} side="top" className="flex-1">
          <button
            onClick={() => setSettingsOpen(!settingsOpen)}
            className={`flex h-9 w-full items-center gap-2 rounded-[3px] px-2.5 text-[13px] font-medium transition-colors cursor-pointer ${
              settingsOpen
                ? 'bg-[var(--bg-active)] text-[var(--text-primary)]'
                : 'text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]'
            }`}
          >
            <Settings size={16} />
            <span>{t('sidebar.settings')}</span>
          </button>
        </ActionTooltip>
        <ActionTooltip content={t('sidebar.lock_vault')} side="top">
          <button
            onClick={() => {
              lockVault();
              navigate('/login');
            }}
            className="flex h-9 w-9 shrink-0 items-center justify-center rounded-[3px] text-[var(--text-secondary)] transition-colors hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)] cursor-pointer"
          >
            <Lock size={16} />
          </button>
        </ActionTooltip>
      </div>

      {/* Resize handle */}
      <div
        className="absolute right-0 top-0 z-10 h-full w-[3px] cursor-col-resize transition-colors hover:bg-[var(--border-focus)]"
        onMouseDown={onResizeStart}
        role="slider"
        aria-label={t('common.resize_sidebar')}
      />

      {/* Edit Tag Modal */}
      <EditTagModal
        open={showEditTag}
        onClose={() => {
          setShowEditTag(false);
          setEditingTag(null);
        }}
        tag={editingTag}
      />

      {/* Tag Context Menu */}
      <TagContextMenu
        open={contextMenu.open}
        x={contextMenu.x}
        y={contextMenu.y}
        onClose={() => setContextMenu((prev) => ({ ...prev, open: false }))}
        onAddPassword={handleContextAddPassword}
        onEdit={handleContextEdit}
        onDelete={handleContextDelete}
      />

      {/* Tags Area Context Menu (Empty Space) */}
      <TagsAreaContextMenu
        open={areaContextMenu.open}
        x={areaContextMenu.x}
        y={areaContextMenu.y}
        onClose={() => setAreaContextMenu((prev) => ({ ...prev, open: false }))}
        onNewTag={() => setIsCreateTagOpen(true)}
        onDeselectAll={() => setFilterCategory('all')}
        sortOrder={settings.tagSortOrder ?? 'custom'}
        onSetSortOrder={(tagSortOrder) => updateSettings({ tagSortOrder })}
        showTagCounts={settings.showTagCounts !== false}
        onToggleShowTagCounts={() => updateSettings({ showTagCounts: !settings.showTagCounts })}
        onDeleteUnusedTags={handleDeleteUnusedTags}
        hasUnusedTags={tags.some((t) => t.count === 0)}
      />

      {/* Delete Tag Modal */}
      <DeleteTagModal
        tag={deleteConfirmTag}
        onClose={() => setDeleteConfirmTag(null)}
        onConfirm={confirmDelete}
      />

    </aside>
  );
}

function NavItem({
  icon,
  label,
  count,
  active,
  density = 'normal',
  onClick,
}: {
  icon: React.ReactNode;
  label: string;
  count: number;
  active: boolean;
  density?: 'compact' | 'normal' | 'comfortable';
  onClick: () => void;
}) {
  let heightClass = 'h-9 text-[13px]';
  if (density === 'compact') heightClass = 'h-7.5 text-[12px]';
  else if (density === 'comfortable') heightClass = 'h-10 text-[14px]';

  return (
    <button
      onClick={onClick}
      className={`flex ${heightClass} w-full items-center gap-2 rounded-[3px] px-2.5 font-medium transition-all cursor-pointer ${
        active
          ? 'bg-[var(--bg-active)] text-[var(--text-primary)]'
          : 'text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]'
      }`}
    >
      <span className={active ? 'text-[var(--text-primary)]' : 'text-[var(--text-secondary)]'}>
        {icon}
      </span>
      <span className="flex-1 text-left">{label}</span>
      <span className="text-[11px] tabular-nums text-[var(--text-tertiary)]">{count}</span>
    </button>
  );
}

function TagItem({
  tag,
  active,
  density = 'normal',
  showCount = true,
  onClick,
  onContextMenu,
  onDoubleClick,
}: {
  tag: Tag;
  active: boolean;
  density?: 'compact' | 'normal' | 'comfortable';
  showCount?: boolean;
  onClick: () => void;
  onContextMenu: (e: React.MouseEvent) => void;
  onDoubleClick: () => void;
}) {
  let heightClass = 'h-9 text-[13px]';
  if (density === 'compact') heightClass = 'h-7.5 text-[12px]';
  else if (density === 'comfortable') heightClass = 'h-10 text-[14px]';

  return (
    <div
      role="button"
      tabIndex={0}
      onClick={onClick}
      onContextMenu={onContextMenu}
      onDoubleClick={onDoubleClick}
      onKeyDown={(e) => {
        if (e.key === 'Enter' || e.key === ' ') {
          e.preventDefault();
          onClick();
        }
      }}
      className={`group flex ${heightClass} w-full items-center gap-2 rounded-[3px] px-2.5 font-medium transition-colors cursor-pointer select-none ${
        active
          ? 'bg-[var(--bg-active)] text-[var(--text-primary)]'
          : 'text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]'
      }`}
    >
      <span
        className="h-2 w-2 shrink-0 rounded-full"
        style={{ backgroundColor: tag.color }}
      />
      <span className="flex-1 truncate text-left">{tag.name}</span>
      {showCount && (
        <span className="text-[11px] tabular-nums text-[var(--text-tertiary)]">{tag.count}</span>
      )}
    </div>
  );
}

export default Sidebar;
