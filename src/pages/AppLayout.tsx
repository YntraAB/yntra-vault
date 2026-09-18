import { useState, useRef, useCallback, useEffect } from 'react';
import { useNavigate } from 'react-router-dom';
import { motion, AnimatePresence } from 'framer-motion';
import { ChevronLeft } from 'lucide-react';
import { useAuth, useAutoLock } from '@/features/auth';
import { useSettings, SettingsPanel } from '@/features/settings';
import { useUi } from '@/contexts/UiContext';
import { useEntries, PasswordList, PasswordDetail, CreateTagModal, EntryModal } from '@/features/entries';
import { useToast } from '@/contexts/ToastContext';
import { useMobile } from '@/hooks/use-mobile';
import { useTranslation } from '@/contexts/LanguageContext';
import { Sidebar, MobileHeader, MobileBottomNav, MobileDrawer, MobileBottomSheet } from '@/components/layout';
import { ToastContainer } from '@/components/ui';
import { VaultTutorial } from '@/components/ui/VaultTutorial';
import { PasswordGenerator } from '@/features/generator';
import { matchesShortcut, getKeybinds } from '@/lib/keybinds';

export default function AppLayout() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const { currentVault, isLocked, lockVault } = useAuth();
  const { settings } = useSettings();
  const {
    settingsOpen,
    setSettingsOpen,
    isEntryModalOpen,
    editingEntry,
    openNewEntryModal,
    closeEntryModal,
    isCreateTagOpen,
    setIsCreateTagOpen,
  } = useUi();
  const { selectedEntry, selectEntryById } = useEntries();
  const { addToast } = useToast();

  const { isMobile } = useMobile();
  const [mobileDrawerOpen, setMobileDrawerOpen] = useState(false);
  const [mobileGeneratorOpen, setMobileGeneratorOpen] = useState(false);
  const [mobileTab, setMobileTab] = useState<'entries' | 'favorites' | 'generator' | 'settings'>('entries');
  const [mobileSearchVisible, setMobileSearchVisible] = useState(false);


  // Global Keybinds (Lock Vault, New Entry)
  useEffect(() => {
    const handleGlobalShortcuts = (e: KeyboardEvent) => {
      const kb = getKeybinds(settings.keybinds);

      // Lock Vault
      if (matchesShortcut(e, kb.lockVault)) {
        e.preventDefault();
        e.stopPropagation();
        lockVault();
        addToast({ message: t('toast.vault_locked'), type: 'info' });
        navigate('/login');
        return;
      }

      if (settingsOpen) return;

      const hasOpenDialog = Boolean(
        document.querySelector('[role="dialog"], [aria-modal="true"], dialog[open], .fixed.inset-0')
      );
      if (hasOpenDialog && !isEntryModalOpen) return;

      // New Entry
      if (matchesShortcut(e, kb.newEntry)) {
        e.preventDefault();
        e.stopPropagation();
        if (!isEntryModalOpen) {
          openNewEntryModal();
        }
        return;
      }
    };

    window.addEventListener('keydown', handleGlobalShortcuts, true);
    return () => window.removeEventListener('keydown', handleGlobalShortcuts, true);
  }, [settings.keybinds, settingsOpen, isEntryModalOpen, openNewEntryModal, lockVault, navigate, addToast, t]);

  // Redirect if not authenticated or locked
  useEffect(() => {
    if (!currentVault) {
      navigate('/');
    } else if (isLocked) {
      navigate('/login');
    }
  }, [currentVault, isLocked, navigate]);

  // Native Android & Browser Back Button / Gesture handling on mobile
  useEffect(() => {
    if (!isMobile) return;

    if (selectedEntry || mobileDrawerOpen || mobileGeneratorOpen || settingsOpen) {
      window.history.pushState({ mobileNav: true }, '');
    }
  }, [isMobile, selectedEntry, mobileDrawerOpen, mobileGeneratorOpen, settingsOpen]);

  useEffect(() => {
    if (!isMobile) return;

    const handlePopState = () => {
      if (mobileDrawerOpen) {
        setMobileDrawerOpen(false);
      } else if (mobileGeneratorOpen) {
        setMobileGeneratorOpen(false);
      } else if (settingsOpen) {
        setSettingsOpen(false);
      } else if (selectedEntry) {
        selectEntryById(null);
      }
    };

    window.addEventListener('popstate', handlePopState);
    return () => window.removeEventListener('popstate', handlePopState);
  }, [isMobile, mobileDrawerOpen, mobileGeneratorOpen, settingsOpen, selectedEntry, setSettingsOpen, selectEntryById]);

  // Configured Inactivity Auto-Lock
  const handleAutoLock = useCallback(() => {
    lockVault();
    addToast({ message: t('toast.vault_locked'), type: 'info' });
    navigate('/login');
  }, [lockVault, addToast, t, navigate]);

  useAutoLock({
    autoLockMinutes: settings.autoLockMinutes,
    isLocked,
    onLock: handleAutoLock,
  });

  // Apply font size & density
  useEffect(() => {
    document.documentElement.style.fontSize = `${settings.fontSize}px`;
    document.documentElement.setAttribute('data-density', settings.density || 'normal');
  }, [settings.fontSize, settings.density]);

  // Resizable panel state (Desktop)
  const sidebarWidthRef = useRef(settings.sidebarWidth);
  const listWidthRef = useRef(settings.passwordListWidth);
  const isDraggingRef = useRef<'sidebar' | 'list' | null>(null);
  const startXRef = useRef(0);
  const startWidthRef = useRef(0);

  const updateCSSVars = useCallback(() => {
    const root = document.documentElement;
    root.style.setProperty('--sidebar-width', `${sidebarWidthRef.current}px`);
    root.style.setProperty('--passwordlist-width', `${listWidthRef.current}px`);
  }, []);

  const handleSidebarResizeStart = useCallback((e: React.MouseEvent) => {
    isDraggingRef.current = 'sidebar';
    startXRef.current = e.clientX;
    startWidthRef.current = sidebarWidthRef.current;
    document.body.style.userSelect = 'none';
    e.preventDefault();
  }, []);

  const handleListResizeStart = useCallback((e: React.MouseEvent) => {
    isDraggingRef.current = 'list';
    startXRef.current = e.clientX;
    startWidthRef.current = listWidthRef.current;
    document.body.style.userSelect = 'none';
    e.preventDefault();
  }, []);

  useEffect(() => {
    const handleMouseMove = (e: MouseEvent) => {
      if (!isDraggingRef.current) return;

      const delta = e.clientX - startXRef.current;

      if (isDraggingRef.current === 'sidebar') {
        const newWidth = Math.min(Math.max(startWidthRef.current + delta, 180), 350);
        sidebarWidthRef.current = newWidth;
      } else if (isDraggingRef.current === 'list') {
        const newWidth = Math.min(Math.max(startWidthRef.current + delta, 220), 450);
        listWidthRef.current = newWidth;
      }

      updateCSSVars();
    };

    const handleMouseUp = () => {
      if (isDraggingRef.current) {
        document.body.style.userSelect = '';
        isDraggingRef.current = null;
      }
    };

    document.addEventListener('mousemove', handleMouseMove);
    document.addEventListener('mouseup', handleMouseUp);

    return () => {
      document.body.style.userSelect = '';
      document.removeEventListener('mousemove', handleMouseMove);
      document.removeEventListener('mouseup', handleMouseUp);
    };
  }, [updateCSSVars]);

  // Initialize CSS vars
  useEffect(() => {
    updateCSSVars();
  }, [updateCSSVars]);

  if (isLocked) {
    return null;
  }

  // Mobile Layout Render Path
  if (isMobile) {
    const showDetail = Boolean(selectedEntry);

    return (
      <div className="flex h-dvh w-dvw flex-col overflow-hidden bg-[var(--bg-base)] text-[var(--text-primary)]">
        <AnimatePresence mode="wait">
          {!showDetail ? (
            /* Mobile Master View: Entry List */
            <motion.div
              key="mobile-list-view"
              initial={{ opacity: 0, x: -15 }}
              animate={{ opacity: 1, x: 0 }}
              exit={{ opacity: 0, x: -15 }}
              transition={{ duration: 0.15 }}
              className="flex flex-1 min-h-0 flex-col overflow-hidden"
            >
              <MobileHeader
                onOpenDrawer={() => setMobileDrawerOpen(true)}
                onNewEntry={openNewEntryModal}
                onToggleSearch={() => setMobileSearchVisible(!mobileSearchVisible)}
                isSearchVisible={mobileSearchVisible}
              />
              <div className="flex-1 min-h-0 overflow-hidden w-full [&>div]:!w-full [&>div]:!border-r-0">
                <PasswordList onResizeStart={() => {}} />
              </div>
            </motion.div>
          ) : (
            /* Mobile Detail View: Selected Password Detail */
            <motion.div
              key="mobile-detail-view"
              initial={{ opacity: 0, x: 20 }}
              animate={{ opacity: 1, x: 0 }}
              exit={{ opacity: 0, x: 20 }}
              transition={{ duration: 0.15 }}
              className="flex flex-1 min-h-0 flex-col overflow-hidden"
            >
              {/* Mobile Back Header */}
              <header className="sticky top-0 z-20 flex w-full shrink-0 flex-col border-b border-[var(--border-subtle)] bg-[var(--bg-surface)]/95 backdrop-blur-md pt-[env(safe-area-inset-top,0px)] select-none">
                <div className="flex h-13 w-full items-center justify-between px-3">
                  <button
                    onClick={() => selectEntryById(null)}
                    className="flex items-center gap-1.5 rounded-[3px] px-2.5 py-1.5 text-[14px] font-medium text-[var(--text-secondary)] active:bg-[var(--bg-hover)] active:text-[var(--text-primary)] cursor-pointer"
                  >
                    <ChevronLeft size={19} />
                    <span>{t('mobile.back_to_vault')}</span>
                  </button>
                </div>
              </header>
              <main className="min-w-0 flex-1 min-h-0 overflow-hidden">
                <PasswordDetail />
              </main>
            </motion.div>
          )}
        </AnimatePresence>

        {/* Mobile Navigation Bottom Bar */}
        <MobileBottomNav
          activeTab={mobileTab}
          onChangeTab={(tab) => {
            setMobileTab(tab);
            if (showDetail) selectEntryById(null);
          }}
          onOpenGenerator={() => setMobileGeneratorOpen(true)}
          onOpenSettings={() => setSettingsOpen(true)}
        />

        {/* Mobile Slide-Over Drawer */}
        <MobileDrawer
          open={mobileDrawerOpen}
          onClose={() => setMobileDrawerOpen(false)}
          onOpenSettings={() => setSettingsOpen(true)}
        />

        {/* Password Generator Mobile Bottom Sheet */}
        <MobileBottomSheet
          open={mobileGeneratorOpen}
          onClose={() => setMobileGeneratorOpen(false)}
          title={t('generator.title')}
        >
          <div className="pb-6">
            <PasswordGenerator />
          </div>
        </MobileBottomSheet>

        <SettingsPanel />
        <ToastContainer />
        <VaultTutorial />
        <CreateTagModal
          open={isCreateTagOpen}
          onClose={() => setIsCreateTagOpen(false)}
        />
        <EntryModal
          open={isEntryModalOpen}
          editEntry={editingEntry}
          onClose={closeEntryModal}
        />
      </div>
    );
  }

  // Desktop Layout Render Path (>= 768px)
  return (
    <div className="flex h-screen w-screen overflow-hidden bg-[var(--bg-base)]">
      <Sidebar onResizeStart={handleSidebarResizeStart} />
      <PasswordList onResizeStart={handleListResizeStart} />
      <main className="min-w-0 flex-1">
        <PasswordDetail />
      </main>
      <SettingsPanel />
      <ToastContainer />
      <VaultTutorial />
      <CreateTagModal
        open={isCreateTagOpen}
        onClose={() => setIsCreateTagOpen(false)}
      />
      <EntryModal
        open={isEntryModalOpen}
        editEntry={editingEntry}
        onClose={closeEntryModal}
      />
    </div>
  );
}




