import { useState, useEffect } from 'react';
import { Key, Star, Sliders, Settings } from 'lucide-react';
import { useAppState } from '@/contexts/AppStateContext';

interface MobileBottomNavProps {
  activeTab: 'entries' | 'favorites' | 'generator' | 'settings';
  onChangeTab: (tab: 'entries' | 'favorites' | 'generator' | 'settings') => void;
  onOpenGenerator: () => void;
  onOpenSettings: () => void;
}

export default function MobileBottomNav({
  activeTab,
  onChangeTab,
  onOpenGenerator,
  onOpenSettings,
}: MobileBottomNavProps) {
  const { setFilterCategory } = useAppState();
  const [isKeyboardOpen, setIsKeyboardOpen] = useState(false);

  useEffect(() => {
    const handleFocusIn = (e: FocusEvent) => {
      const target = e.target as HTMLElement | null;
      if (target && (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA')) {
        setIsKeyboardOpen(true);
      }
    };
    const handleFocusOut = (e: FocusEvent) => {
      const target = e.target as HTMLElement | null;
      if (target && (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA')) {
        setIsKeyboardOpen(false);
      }
    };

    window.addEventListener('focusin', handleFocusIn);
    window.addEventListener('focusout', handleFocusOut);
    return () => {
      window.removeEventListener('focusin', handleFocusIn);
      window.removeEventListener('focusout', handleFocusOut);
    };
  }, []);

  if (isKeyboardOpen) return null;

  return (
    <nav className="fixed bottom-0 left-0 right-0 z-30 flex h-16 w-full items-center justify-around border-t border-[var(--border-subtle)] bg-[var(--bg-surface)] px-2 pb-[env(safe-area-inset-bottom,0px)] shadow-lg select-none">
      <button
        onClick={() => {
          setFilterCategory('all');
          onChangeTab('entries');
        }}
        className={`flex flex-1 flex-col items-center justify-center gap-1 py-1.5 transition-colors ${
          activeTab === 'entries'
            ? 'text-[var(--text-primary)] font-semibold'
            : 'text-[var(--text-tertiary)] hover:text-[var(--text-secondary)]'
        }`}
      >
        <Key size={20} className={activeTab === 'entries' ? 'stroke-[2.5]' : 'stroke-[1.75]'} />
        <span className="text-[10px]">Vault</span>
      </button>

      <button
        onClick={() => {
          setFilterCategory('favorites');
          onChangeTab('favorites');
        }}
        className={`flex flex-1 flex-col items-center justify-center gap-1 py-1.5 transition-colors ${
          activeTab === 'favorites'
            ? 'text-orange-400 font-semibold'
            : 'text-[var(--text-tertiary)] hover:text-[var(--text-secondary)]'
        }`}
      >
        <Star size={20} className={activeTab === 'favorites' ? 'fill-current stroke-none' : 'stroke-[1.75]'} />
        <span className="text-[10px]">Favorites</span>
      </button>

      <button
        onClick={() => {
          onOpenGenerator();
        }}
        className="flex flex-1 flex-col items-center justify-center gap-1 py-1.5 text-[var(--text-tertiary)] hover:text-[var(--text-secondary)] transition-colors"
      >
        <Sliders size={20} className="stroke-[1.75]" />
        <span className="text-[10px]">Generator</span>
      </button>

      <button
        onClick={() => {
          onOpenSettings();
        }}
        className="flex flex-1 flex-col items-center justify-center gap-1 py-1.5 text-[var(--text-tertiary)] hover:text-[var(--text-secondary)] transition-colors"
      >
        <Settings size={20} className="stroke-[1.75]" />
        <span className="text-[10px]">Settings</span>
      </button>
    </nav>
  );
}
