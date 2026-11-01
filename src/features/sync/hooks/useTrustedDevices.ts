import { useCallback, useEffect, useRef, useState } from 'react';
import { useAuth } from '@/features/auth';
import { useToast } from '@/contexts/ToastContext';
import type { TrustedDevice } from '@/lib/backend';

export function useTrustedDevices() {
  const { backend, currentVault, getSessionGeneration, isSessionCurrent } = useAuth();
  const { addToast } = useToast();
  const [trustedDevices, setTrustedDevices] = useState<TrustedDevice[]>([]);
  const [isLoadingDevices, setIsLoadingDevices] = useState(true);
  const alive = useRef(false);
  const sequence = useRef(0);
  const loadTrustedDevices = useCallback(() => {
    if (!backend || !currentVault) return Promise.resolve();
    const generation = getSessionGeneration(), request = ++sequence.current;
    const valid = () => alive.current && request === sequence.current && isSessionCurrent(generation);
    return backend.getTrustedDevices()
      .then(devices => { if (valid()) setTrustedDevices(devices); })
      .catch(error => { if (valid()) addToast({ message: String(error), type: 'error' }); })
      .finally(() => { if (valid()) setIsLoadingDevices(false); });
  }, [backend, currentVault, getSessionGeneration, isSessionCurrent, addToast]);
  useEffect(() => {
    alive.current = true;
    let disposed = false;
    let unsubscribe: (() => void) | undefined;
    void loadTrustedDevices();
    if (backend?.onSyncOperation) {
      void backend.onSyncOperation(operation => {
        if (!disposed && (operation.phase === 'complete' || operation.phase === 'incomplete')) void loadTrustedDevices();
      }).then(stop => {
        if (disposed) stop(); else { unsubscribe = stop; void loadTrustedDevices(); }
      }).catch(error => { if (!disposed) addToast({ message: String(error), type: 'error' }); });
    }
    return () => { disposed = true; alive.current = false; unsubscribe?.(); };
  }, [backend, loadTrustedDevices, addToast]);
  return { trustedDevices, isLoadingDevices, loadTrustedDevices };
}
