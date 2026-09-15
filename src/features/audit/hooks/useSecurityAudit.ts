import { useState, useCallback, useEffect } from 'react';
import type { SecurityAudit } from '@/lib/backend';
import { useBackend } from '@/lib/useBackend';

let globalAudit: SecurityAudit | null = null;
const auditListeners = new Set<(audit: SecurityAudit | null) => void>();

export function useSecurityAudit() {
  const { backend } = useBackend();
  const [audit, setAudit] = useState<SecurityAudit | null>(globalAudit);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    const listener = (newAudit: SecurityAudit | null) => {
      setAudit(newAudit);
    };
    auditListeners.add(listener);
    return () => {
      auditListeners.delete(listener);
    };
  }, []);

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
      globalAudit = result;
      setAudit(result);
      auditListeners.forEach((fn) => fn(result));
      return result;
    } finally {
      if (!silent) setLoading(false);
    }
  }, [backend]);

  return { audit, loading, runAudit };
}

export default useSecurityAudit;

