import { useCallback, useRef, useState } from 'react';

export function useMutationGate() {
  const pending = useRef(false);
  const [busy, setBusy] = useState(false);
  const run = useCallback(async (action: () => Promise<boolean>) => {
    if (pending.current) return false;
    pending.current = true; setBusy(true);
    try { return await action(); }
    finally { pending.current = false; setBusy(false); }
  }, []);
  const close = useCallback((action: () => void) => { if (!pending.current) action(); }, []);
  return { busy, run, close };
}
