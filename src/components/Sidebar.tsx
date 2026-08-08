import { useState, useCallback, useMemo } from 'react';
import { Globe, Star, Plus, Settings, Lock } from 'lucide-react';
import { motion } from 'framer-motion';
import { useNavigate } from 'react-router-dom';
import { useAppState } from '@/contexts/AppStateContext';
import { useTranslation } from '@/contexts/LanguageContext';
import CreateTagModal from './CreateTagModal';
import EditTagModal from './EditTagModal';
import TagContextMenu from './TagContextMenu';
import TagsAreaContextMenu from './TagsAreaContextMenu';
import type { Tag } from '@/types';
import { Skeleton } from './ui/skeleton';
import { ActionTooltip } from './ui/tooltip';

interface SidebarProps {
  onResizeStart: (e: React.MouseEvent) => void;
}

const containerVariants = {
  hidden: { opacity: 0 },
  show: { opacity: 1, transition: { staggerChildren: 0.02 } },
};

const itemVariants = {
  hidden: { opacity: 0 },
  show: { opacity: 1, transition: { duration: 0.1 } },
};

export default function Sidebar({ onResizeStart }: SidebarProps) {
  const navigate = useNavigate();
  const { t } = useTranslation();
  const {
    tags,
    filterCategory,
    setFilterCategory,
    entries,
    settingsOpen,
    setSettingsOpen,
    setIsLocked,
    removeTag,
    addToast,
    isLoadingEntries,
    setIsEntryModalOpen,
    settings,
    updateSettings,
  } = useAppState();

  const allCount = entries.length;
  const favCount = entries.filter((e) => e.favorite).length;

  const sortedTags = useMemo(() => {
    const list = [...tags];
    const order = settings.tagSortOrder ?? 'name';
    if (order === 'count') {
      return list.sort((a, b) => b.count - a.count || a.name.localeCompare(b.name));
    }
    return list.sort((a, b) => a.name.localeCompare(b.name));
  }, [tags, settings.tagSortOrder]);

  // Modal state
  const [showCreateTag, setShowCreateTag] = useState(false);
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
      // If the deleted tag is currently selected, reset to 'all'
      if (filterCategory === deleteConfirmTag.name) {
        setFilterCategory('all');
      }
      removeTag(deleteConfirmTag.id);
      addToast({ message: `Tag "${deleteConfirmTag.name}" deleted`, type: 'info' });
      setDeleteConfirmTag(null);
    }
  }, [deleteConfirmTag, removeTag, addToast, filterCategory, setFilterCategory]);

  return (
    <aside
      className="relative flex h-full flex-col border-r border-[var(--border-subtle)] bg-[var(--bg-surface)]"
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
              onClick={() => setShowCreateTag(true)}
              className="inline-flex items-center justify-center rounded-[3px] p-1 text-[var(--text-tertiary)] transition-colors hover:text-[var(--text-primary)]"
            >
              <Plus size={14} />
            </button>
          </ActionTooltip>
        </div>

        <motion.div
          className="flex flex-1 flex-col gap-[2px] overflow-y-auto p-2 pt-0"
          variants={containerVariants}
          initial="hidden"
          animate="show"
        >
          {isLoadingEntries ? (
            <div className="flex flex-col gap-[6px] px-2.5 py-1.5">
              {[...Array(4)].map((_, i) => (
                <div key={i} className="flex h-7 items-center gap-2">
                  <Skeleton className="h-2.5 w-2.5 rounded-full shrink-0" />
                  <Skeleton className="h-3.5 w-16" />
                </div>
              ))}
            </div>
          ) : (
            sortedTags.map((tag: Tag) => (
              <motion.div key={tag.id} variants={itemVariants}>
                <TagItem
                  tag={tag}
                  active={filterCategory === tag.name}
                  density={settings.density}
                  showCount={settings.showTagCounts !== false}
                  onClick={() => setFilterCategory(tag.name)}
                  onContextMenu={(e) => handleTagContextMenu(e, tag)}
                  onDoubleClick={() => {
                    setEditingTag(tag);
                    setShowEditTag(true);
                  }}
                />
              </motion.div>
            ))
          )}
        </motion.div>
      </div>

      {/* Footer: Settings + Lock */}
      <div className="flex shrink-0 gap-1 border-t border-[var(--border-subtle)] p-2">
        <ActionTooltip content={t('sidebar.settings')} side="top" className="flex-1">
          <button
            onClick={() => setSettingsOpen(!settingsOpen)}
            className={`flex h-9 w-full items-center gap-2 rounded-[3px] px-2.5 text-[13px] font-medium transition-colors ${
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
              setIsLocked(true);
              navigate('/login');
            }}
            className="flex h-9 w-9 shrink-0 items-center justify-center rounded-[3px] text-[var(--text-secondary)] transition-colors hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"
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
        aria-label="Resize sidebar"
      />

      {/* Create Tag Modal */}
      <CreateTagModal
        open={showCreateTag}
        onClose={() => setShowCreateTag(false)}
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
        onNewTag={() => setShowCreateTag(true)}
        onDeselectAll={() => setFilterCategory('all')}
        sortOrder={settings.tagSortOrder ?? 'name'}
        onSetSortOrder={(tagSortOrder) => updateSettings({ tagSortOrder })}
        showTagCounts={settings.showTagCounts !== false}
        onToggleShowTagCounts={() => updateSettings({ showTagCounts: !settings.showTagCounts })}
        onDeleteUnusedTags={handleDeleteUnusedTags}
        hasUnusedTags={tags.some((t) => t.count === 0)}
      />

      {/* Delete Confirmation Overlay */}
      {deleteConfirmTag && (
        <div
          className="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
          onClick={() => setDeleteConfirmTag(null)}
        >
          <motion.div
            initial={{ scale: 0.98, opacity: 0 }}
            animate={{ scale: 1, opacity: 1 }}
            transition={{ duration: 0.15 }}
            className="w-[360px] rounded-[4px] border border-[var(--border)] bg-[var(--bg-elevated)] p-5"
            onClick={(e) => e.stopPropagation()}
          >
            <h3 className="text-[16px] font-semibold text-[var(--text-primary)]">
              Delete Tag
            </h3>
            <p className="mt-2 text-[13px] text-[var(--text-secondary)]">
              Are you sure you want to delete &ldquo;{deleteConfirmTag.name}&rdquo;?
              {deleteConfirmTag.count > 0 && (
                <> It will be removed from {deleteConfirmTag.count} {deleteConfirmTag.count === 1 ? 'entry' : 'entries'}.</>
              )}
            </p>
            <div className="mt-4 flex justify-end gap-2">
              <button
                onClick={() => setDeleteConfirmTag(null)}
                className="h-8 rounded-[3px] px-3 text-[13px] font-medium text-[var(--text-secondary)] transition-colors hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"
              >
                Cancel
              </button>
              <button
                onClick={confirmDelete}
                className="h-8 rounded-[3px] bg-[var(--destructive)] px-3 text-[13px] font-medium text-white transition-colors hover:opacity-90"
              >
                Delete
              </button>
            </div>
          </motion.div>
        </div>
      )}
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
      className={`flex ${heightClass} w-full items-center gap-2 rounded-[3px] px-2.5 font-medium transition-all ${
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
    <button
      onClick={onClick}
      onContextMenu={onContextMenu}
      onDoubleClick={onDoubleClick}
      className={`flex ${heightClass} w-full items-center gap-2 rounded-[3px] px-2.5 font-medium transition-all ${
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
    </button>
  );
}



