import { useState, useCallback } from 'react';
import { Keyboard } from 'lucide-react';
import { useBackend } from '@/lib/useBackend';
import { useAppState } from '@/contexts/AppStateContext';

interface AutotypeButtonProps {
  value: string;
  className?: string;
  size?: number;
}

export default function AutotypeButton({ value, className = '', size = 14 }: AutotypeButtonProps) {
  const { backend } = useBackend();
  const { addToast, settings } = useAppState();
  const [autotyping, setAutotyping] = useState(false);

  const handleAutotype = useCallback(
    (e: React.MouseEvent) => {
      e.stopPropagation();
      if (!backend || autotyping) return;
      setAutotyping(true);
      addToast({ message: 'Autotype pending... Focus another window to start typing.', type: 'info' });

      (async () => {
        try {
          await backend.autotype(value, settings.autotypeCharDelayMs || 15);
          addToast({ message: 'Autotyped successfully', type: 'success' });
        } catch (err) {
          addToast({ message: `Autotype failed: ${err}`, type: 'error' });
        } finally {
          setAutotyping(false);
        }
      })();
    },
    [backend, autotyping, value, addToast, settings.autotypeCharDelayMs]
  );

  return (
    <button
      onClick={handleAutotype}
      disabled={autotyping}
      className={`inline-flex items-center justify-center rounded-[3px] p-1 text-[var(--text-tertiary)] transition-all duration-100 hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)] active:scale-95 disabled:opacity-50 disabled:scale-100 ${className}`}
      title="Autotype"
    >
      <Keyboard size={size} className={autotyping ? 'animate-pulse text-[var(--text-primary)]' : ''} />
    </button>
  );
}
