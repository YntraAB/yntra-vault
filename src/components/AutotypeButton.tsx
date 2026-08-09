import { useState, useCallback } from 'react';
import { Keyboard } from 'lucide-react';
import { useBackend } from '@/lib/useBackend';
import { useAppState } from '@/contexts/AppStateContext';
import { useTranslation } from '@/contexts/LanguageContext';
import { ActionTooltip } from './ui/tooltip';
import { isTauri } from '@/lib/backend';

interface AutotypeButtonProps {
  value?: string;
  entryId?: string;
  className?: string;
  size?: number;
}

export default function AutotypeButton({ value = '', entryId, className = '', size = 14 }: AutotypeButtonProps) {
  const { t } = useTranslation();
  const { backend } = useBackend();
  const { addToast, settings } = useAppState();
  const [autotyping, setAutotyping] = useState(false);

  const handleAutotype = useCallback(
    (e: React.MouseEvent) => {
      e.stopPropagation();
      if (!backend || autotyping) return;
      setAutotyping(true);
      const settleSeconds = Math.round((settings.autotypeSettleDelayMs || 3000) / 1000);
      addToast({ message: `Autotype pending... Focus target window. You will have ${settleSeconds} seconds to select the input field.`, type: 'info' });

      (async () => {
        try {
          if (isTauri() && entryId) {
            await backend.autotypeEntryPassword(entryId, settings.autotypeCharDelayMs || 15, settings.autotypeSettleDelayMs || 3000);
          } else {
            await backend.autotype(value, settings.autotypeCharDelayMs || 15, settings.autotypeSettleDelayMs || 3000);
          }
          addToast({ message: t('toast.autotyped_success'), type: 'success' });
        } catch (err) {
          addToast({ message: `Autotype failed: ${err}`, type: 'error' });
        } finally {
          setAutotyping(false);
        }
      })();
    },
    [backend, autotyping, value, entryId, addToast, settings.autotypeCharDelayMs, settings.autotypeSettleDelayMs]
  );

  return (
    <ActionTooltip content={autotyping ? t('autotype.autotyping') : t('autotype.tooltip')}>
      <button
        type="button"
        onClick={handleAutotype}
        disabled={autotyping}
        className={`inline-flex items-center justify-center rounded-[3px] p-1 text-[var(--text-tertiary)] transition-all duration-100 hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)] active:scale-95 disabled:opacity-50 disabled:scale-100 ${className}`}
      >
        <Keyboard size={size} className={autotyping ? 'animate-pulse text-[var(--text-primary)]' : ''} />
      </button>
    </ActionTooltip>
  );
}
