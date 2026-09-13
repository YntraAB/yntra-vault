import { useState, useCallback } from 'react';
import type { SecurityAudit } from '@/lib/backend';
import { useBackend } from '@/lib/useBackend';

export function useSecurityAudit() {
  const { backend } = useBackend();
  const [audit, setAudit] = useState<SecurityAudit | null>(null);
  const [loading, setLoading] = useState(false);

  const runAudit = useCallback(async (disableSkeletonDelays = false, silent = false) => {
    if (!backend) return;
    if (!silent) setLoading(true);
    const startTime = Date.now();
    try {
      const result = await backend.securityAudit();
      const elapsed = Date.now() - startTime;
      if (!disableSkeletonDelays && !silent && elapsed < 250) {
        await new Promise((resolve) => setTimeout(resolve, 250 - elapsed));
      }
      setAudit(result);
      return result;
    } finally {
      if (!silent) setLoading(false);
    }
  }, [backend]);

  return { audit, loading, runAudit };
}

export default useSecurityAudit;
