import { useEffect, type RefObject } from 'react';

export function useModalFocus(active: boolean, container: RefObject<HTMLElement | null>, onClose: () => void) {
  useEffect(() => {
    const element = container.current;
    if (!active || !element) return;
    const previous = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    const controls = () => Array.from(element.querySelectorAll<HTMLElement>('button:not(:disabled), input:not(:disabled), select:not(:disabled), textarea:not(:disabled), a[href], [tabindex="0"]'))
      .filter(control => !control.closest('[inert], [hidden], fieldset:disabled'));
    (controls()[0] ?? element).focus();
    const keydown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        event.preventDefault(); event.stopPropagation(); onClose();
      } else if (event.key === 'Tab') {
        const available = controls();
        const first = available[0] ?? element;
        const last = available.at(-1) ?? element;
        if (!element.contains(document.activeElement) || (event.shiftKey && document.activeElement === first) || (!event.shiftKey && document.activeElement === last) || available.length === 0) {
          event.preventDefault(); (event.shiftKey ? last : first).focus();
        }
      }
    };
    element.addEventListener('keydown', keydown);
    return () => { element.removeEventListener('keydown', keydown); if (previous?.isConnected) previous.focus(); };
  }, [active, container, onClose]);
}
