import { useRef, useEffect } from 'react';
import { Menu, Plus, Search, ShieldCheck, X } from 'lucide-react';
import { useAuth } from '@/features/auth';
import { useUi, useSearch } from '@/contexts/UiContext';
import { useTranslation } from '@/contexts/LanguageContext';

export interface MobileHeaderProps {
  onOpenDrawer: () => void;
  onNewEntry: () => void;
  onToggleSearch: () => void;
  isSearchVisible: boolean;
}

export function MobileHeader({
  onOpenDrawer,
  onNewEntry,
  onToggleSearch,
  isSearchVisible,
}: MobileHeaderProps) {
  const { t } = useTranslation();
  const { currentVault } = useAuth();
  const { filterCategory } = useUi();
  const { searchTerm, setSearchTerm } = useSearch();
  const searchInputRef = useRef<HTMLInputElement>(null);

  const title = filterCategory === 'all' 
    ? (currentVault?.name || 'Yntra Vault')
    : filterCategory === 'favorites' 
    ? t('sidebar.favorites')
    : filterCategory;

  useEffect(() => {
    if (isSearchVisible && searchInputRef.current) {
      searchInputRef.current.focus();
    }
  }, [isSearchVisible]);

  return (
    <header className="sticky top-0 z-30 flex w-full shrink-0 flex-col border-b border-[var(--border-subtle)] bg-[var(--bg-surface)]/95 backdrop-blur-md pt-[env(safe-area-inset-top,0px)] select-none">
      <div className="flex h-14 w-full items-center justify-between px-3">
        {/* Menu / Drawer Toggle */}
        <button
          onClick={onOpenDrawer}
          aria-label={t('mobile.open_drawer')}
          className="flex h-9 w-9 items-center justify-center rounded-lg text-[var(--text-secondary)] transition-colors active:bg-[var(--bg-hover)] active:text-[var(--text-primary)] cursor-pointer shrink-0"
        >
          <Menu size={20} />
        </button>

        {/* Header Title */}
        <div className="flex flex-1 items-center justify-center px-2 min-w-0">
          <div className="flex items-center gap-1.5 min-w-0">
            <ShieldCheck size={16} className="text-[var(--text-primary)] shrink-0" />
            <span className="truncate text-[15px] font-semibold tracking-tight text-[var(--text-primary)]">
              {title}
            </span>
          </div>
        </div>

        {/* Action Buttons */}
        <div className="flex items-center gap-1.5 shrink-0">
          <button
            onClick={onToggleSearch}
            aria-label={t('mobile.search')}
            className={`flex h-9 w-9 items-center justify-center rounded-lg transition-colors cursor-pointer ${
              isSearchVisible
                ? 'bg-[var(--bg-active)] text-[var(--text-primary)]'
                : 'text-[var(--text-secondary)] active:bg-[var(--bg-hover)] active:text-[var(--text-primary)]'
            }`}
          >
            <Search size={19} />
          </button>

          {/* New Entry: Redesigned with elevated background and crisp border to blend harmoniously */}
          <button
            onClick={onNewEntry}
            aria-label={t('mobile.new_entry')}
            className="flex h-9 w-9 items-center justify-center rounded-lg border border-[var(--border)] bg-[var(--bg-elevated)] text-[var(--text-primary)] transition-all active:scale-95 active:bg-[var(--bg-active)] hover:border-[var(--border-focus)] shadow-xs cursor-pointer"
          >
            <Plus size={19} />
          </button>
        </div>
      </div>

      {/* Expandable Mobile Search Bar */}
      {isSearchVisible && (
        <div className="flex h-12 w-full items-center px-3 pb-2 pt-0">
          <div className="flex h-9 w-full items-center gap-2 rounded-lg border border-[var(--border)] bg-[var(--bg-elevated)] px-3 focus-within:border-[var(--border-focus)]">
            <Search size={15} className="shrink-0 text-[var(--text-tertiary)]" />
            <input
              ref={searchInputRef}
              type="text"
              placeholder={t('app.search_placeholder')}
              value={searchTerm}
              onChange={(e) => setSearchTerm(e.target.value)}
              className="flex-1 bg-transparent text-[13px] text-[var(--text-primary)] outline-none placeholder:text-[var(--text-tertiary)]"
            />
            {searchTerm && (
              <button
                onClick={() => setSearchTerm('')}
                className="flex h-6 w-6 items-center justify-center rounded-md text-[var(--text-tertiary)] hover:text-[var(--text-primary)] cursor-pointer"
              >
                <X size={14} />
              </button>
            )}
          </div>
        </div>
      )}
    </header>
  );
}

export default MobileHeader;
