import { useRef, useCallback, useEffect } from 'react';
import { useNavigate } from 'react-router-dom';
import { useAppState } from '@/contexts/AppStateContext';
import Sidebar from '@/components/Sidebar';
import PasswordList from '@/components/PasswordList';
import PasswordDetail from '@/components/PasswordDetail';
import SettingsPanel from '@/components/SettingsPanel';
import ToastContainer from '@/components/ToastContainer';
import { isTauri } from '@/lib/backend';

import { matchesShortcut, getKeybinds } from '@/lib/keybinds';

export default function AppLayout() {
  const navigate = useNavigate();
  const { currentVault, isLocked, setIsLocked, settings, settingsOpen, isEntryModalOpen, setIsEntryModalOpen, addToast } = useAppState();
  const autoLockTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Global Keybinds (Lock Vault, New Entry)
  useEffect(() => {
    const handleGlobalShortcuts = (e: KeyboardEvent) => {
      const kb = getKeybinds(settings.keybinds);

      // Lock Vault
      if (matchesShortcut(e, kb.lockVault)) {
        e.preventDefault();
        e.stopPropagation();
        setIsLocked(true);
        addToast({ message: 'Vault locked', type: 'info' });
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
          setIsEntryModalOpen(true);
        }
        return;
      }
    };

    window.addEventListener('keydown', handleGlobalShortcuts, true);
    return () => window.removeEventListener('keydown', handleGlobalShortcuts, true);
  }, [settings.keybinds, settingsOpen, isEntryModalOpen, setIsEntryModalOpen, setIsLocked, navigate, addToast]);

  // Redirect if not authenticated or not in Tauri desktop mode
  useEffect(() => {
    if (!isTauri() || !currentVault) {
      navigate('/');
    }
  }, [currentVault, navigate]);

  // Auto-lock timer
  useEffect(() => {
    if (settings.autoLockMinutes <= 0) return;

    const resetTimer = () => {
      if (autoLockTimerRef.current) clearTimeout(autoLockTimerRef.current);
      autoLockTimerRef.current = setTimeout(() => {
        setIsLocked(true);
        navigate('/login');
      }, settings.autoLockMinutes * 60 * 1000);
    };

    // Reset on user activity
    const events = ['mousemove', 'keypress', 'click', 'scroll'];
    events.forEach(e => window.addEventListener(e, resetTimer, { passive: true }));
    resetTimer();

    return () => {
      if (autoLockTimerRef.current) clearTimeout(autoLockTimerRef.current);
      events.forEach(e => window.removeEventListener(e, resetTimer));
    };
  }, [settings.autoLockMinutes, setIsLocked, navigate]);

  // Apply font size & density
  useEffect(() => {
    document.documentElement.style.fontSize = `${settings.fontSize}px`;
    document.documentElement.setAttribute('data-density', settings.density || 'normal');
  }, [settings.fontSize, settings.density]);

  // Resizable panel state
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

  const handleSidebarResizeStart = useCallback(
    (e: React.MouseEvent) => {
      isDraggingRef.current = 'sidebar';
      startXRef.current = e.clientX;
      startWidthRef.current = sidebarWidthRef.current;
      e.preventDefault();
    },
    []
  );

  const handleListResizeStart = useCallback(
    (e: React.MouseEvent) => {
      isDraggingRef.current = 'list';
      startXRef.current = e.clientX;
      startWidthRef.current = listWidthRef.current;
      e.preventDefault();
    },
    []
  );

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
      isDraggingRef.current = null;
    };

    document.addEventListener('mousemove', handleMouseMove);
    document.addEventListener('mouseup', handleMouseUp);

    return () => {
      document.removeEventListener('mousemove', handleMouseMove);
      document.removeEventListener('mouseup', handleMouseUp);
    };
  }, [updateCSSVars]);

  // Initialize CSS vars
  useEffect(() => {
    updateCSSVars();
  }, [updateCSSVars]);

  if (isLocked) {
    navigate('/login');
    return null;
  }

  return (
    <div className="flex h-screen w-screen overflow-hidden bg-[var(--bg-base)]">
      <Sidebar onResizeStart={handleSidebarResizeStart} />
      <PasswordList onResizeStart={handleListResizeStart} />
      <main className="min-w-0 flex-1">
        <PasswordDetail />
      </main>
      <SettingsPanel />
      <ToastContainer />
    </div>
  );
}



