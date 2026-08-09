import { useState } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { Globe, Star, Plus, Settings, Lock, X, Database, Shield } from 'lucide-react';
import { useNavigate } from 'react-router-dom';
import { useAppState } from '@/contexts/AppStateContext';
import { useTranslation } from '@/contexts/LanguageContext';
import CreateTagModal from '@/components/CreateTagModal';

interface MobileDrawerProps {
  open: boolean;
  onClose: () => void;
  onOpenSettings: () => void;
}

export default function MobileDrawer({ open, onClose, onOpenSettings }: MobileDrawerProps) {
  const navigate = useNavigate();
  const { t } = useTranslation();
  const {
    currentVault,
    tags,
    filterCategory,
    setFilterCategory,
    entries,
    setIsLocked,
    setCurrentVault,
  } = useAppState();

  const [showCreateTag, setShowCreateTag] = useState(false);

  const allCount = entries.length;
  const favCount = entries.filter((e) => e.favorite).length;

  if (!open) return null;

  return (
    <AnimatePresence>
      <div className="fixed inset-0 z-50 flex">
        {/* Backdrop */}
        <motion.div
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          onClick={onClose}
          className="fixed inset-0 bg-black/60 backdrop-blur-xs"
        />

        {/* Drawer Content */}
        <motion.aside
          initial={{ x: '-100%' }}
          animate={{ x: 0 }}
          exit={{ x: '-100%' }}
          transition={{ type: 'spring', damping: 25, stiffness: 280 }}
          className="relative z-10 flex h-full w-[280px] max-w-[85vw] flex-col border-r border-[var(--border)] bg-[var(--bg-surface)] px-4 pt-[env(safe-area-inset-top,0px)] pb-[env(safe-area-inset-bottom,0px)] text-[var(--text-primary)] shadow-2xl select-none"
        >
          {/* Header */}
          <div className="flex h-14 items-center justify-between border-b border-[var(--border-subtle)]">
            <div className="flex items-center gap-2 min-w-0">
              <div className="flex h-8 w-8 items-center justify-center rounded-lg bg-[var(--text-primary)]/10 text-[var(--text-primary)]">
                <Shield size={18} />
              </div>
              <div className="flex flex-col min-w-0">
                <span className="truncate text-[14px] font-semibold">
                  {currentVault?.name || 'Yntra Vault'}
                </span>
                <span className="text-[11px] text-[var(--text-tertiary)]">Zero-Knowledge Vault</span>
              </div>
            </div>
            <button
              onClick={onClose}
              className="flex h-9 w-9 items-center justify-center rounded-lg text-[var(--text-tertiary)] active:bg-[var(--bg-hover)] active:text-[var(--text-primary)]"
            >
              <X size={18} />
            </button>
          </div>

          {/* Navigation Links */}
          <div className="mt-4 flex flex-col gap-1">
            <button
              onClick={() => {
                setFilterCategory('all');
                onClose();
              }}
              className={`flex h-11 w-full items-center justify-between rounded-lg px-3 text-[14px] font-medium transition-colors ${
                filterCategory === 'all'
                  ? 'bg-[var(--bg-active)] text-[var(--text-primary)]'
                  : 'text-[var(--text-secondary)] active:bg-[var(--bg-hover)] active:text-[var(--text-primary)]'
              }`}
            >
              <div className="flex items-center gap-3">
                <Globe size={18} />
                <span>{t('sidebar.all_items')}</span>
              </div>
              <span className="text-[12px] font-semibold text-[var(--text-tertiary)]">{allCount}</span>
            </button>

            <button
              onClick={() => {
                setFilterCategory('favorites');
                onClose();
              }}
              className={`flex h-11 w-full items-center justify-between rounded-lg px-3 text-[14px] font-medium transition-colors ${
                filterCategory === 'favorites'
                  ? 'bg-[var(--bg-active)] text-orange-400'
                  : 'text-[var(--text-secondary)] active:bg-[var(--bg-hover)] active:text-[var(--text-primary)]'
              }`}
            >
              <div className="flex items-center gap-3">
                <Star size={18} className="text-orange-400" />
                <span>{t('sidebar.favorites')}</span>
              </div>
              <span className="text-[12px] font-semibold text-[var(--text-tertiary)]">{favCount}</span>
            </button>
          </div>

          {/* Tags List */}
          <div className="mt-6 flex flex-1 flex-col overflow-hidden">
            <div className="flex items-center justify-between px-2 pb-2">
              <span className="text-[11px] font-semibold uppercase tracking-wider text-[var(--text-tertiary)]">
                {t('sidebar.tags')}
              </span>
              <button
                onClick={() => setShowCreateTag(true)}
                className="flex h-7 w-7 items-center justify-center rounded-md text-[var(--text-tertiary)] active:bg-[var(--bg-hover)] active:text-[var(--text-primary)]"
              >
                <Plus size={16} />
              </button>
            </div>

            <div className="flex flex-1 flex-col gap-1 overflow-y-auto pr-1">
              {tags.map((tag) => (
                <button
                  key={tag.id}
                  onClick={() => {
                    setFilterCategory(tag.name);
                    onClose();
                  }}
                  className={`flex h-10 w-full items-center justify-between rounded-lg px-3 text-[13px] font-medium transition-colors ${
                    filterCategory === tag.name
                      ? 'bg-[var(--bg-active)] text-[var(--text-primary)]'
                      : 'text-[var(--text-secondary)] active:bg-[var(--bg-hover)] active:text-[var(--text-primary)]'
                  }`}
                >
                  <div className="flex items-center gap-2.5 min-w-0">
                    <span
                      className="h-2.5 w-2.5 rounded-full shrink-0"
                      style={{ backgroundColor: tag.color }}
                    />
                    <span className="truncate">{tag.name}</span>
                  </div>
                  <span className="text-[11px] text-[var(--text-tertiary)]">{tag.count}</span>
                </button>
              ))}
            </div>
          </div>

          {/* Footer Actions */}
          <div className="mt-auto border-t border-[var(--border-subtle)] pt-3 pb-2 flex flex-col gap-1">
            <button
              onClick={() => {
                onClose();
                onOpenSettings();
              }}
              className="flex h-11 w-full items-center gap-3 rounded-lg px-3 text-[13px] font-medium text-[var(--text-secondary)] active:bg-[var(--bg-hover)] active:text-[var(--text-primary)]"
            >
              <Settings size={18} />
              <span>{t('sidebar.settings')}</span>
            </button>

            <button
              onClick={() => {
                onClose();
                setCurrentVault(null);
                navigate('/');
              }}
              className="flex h-11 w-full items-center gap-3 rounded-lg px-3 text-[13px] font-medium text-[var(--text-secondary)] active:bg-[var(--bg-hover)] active:text-[var(--text-primary)]"
            >
              <Database size={18} />
              <span>Switch Vault</span>
            </button>

            <button
              onClick={() => {
                onClose();
                setIsLocked(true);
                navigate('/login');
              }}
              className="flex h-11 w-full items-center gap-3 rounded-lg px-3 text-[13px] font-medium text-[var(--destructive)] active:bg-[var(--destructive)]/10"
            >
              <Lock size={18} />
              <span>{t('sidebar.lock_vault')}</span>
            </button>
          </div>
        </motion.aside>

        <CreateTagModal
          open={showCreateTag}
          onClose={() => setShowCreateTag(false)}
        />
      </div>
    </AnimatePresence>
  );
}
