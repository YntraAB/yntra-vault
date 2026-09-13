import { useState, useEffect, useCallback, useRef } from 'react';
import { useBackend } from '@/lib/useBackend';
import type { TotpCode } from '@/lib/backend';

export function useTotp(secret: string | null) {
  const { backend } = useBackend();
  const [code, setCode] = useState<TotpCode | null>(null);
  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null);

  const generate = useCallback(async () => {
    if (!backend || !secret) return;
    try {
      const result = await backend.generateTotp(secret);
      setCode(result);
    } catch {
      setCode(null);
    }
  }, [backend, secret]);

  useEffect(() => {
    if (!secret) {
      setCode(null);
      return;
    }

    generate();

    intervalRef.current = setInterval(() => {
      setCode((prev) => {
        if (!prev) return null;
        const remaining = prev.seconds_remaining - 1;
        if (remaining <= 0) {
          generate();
          return prev;
        }
        return { ...prev, seconds_remaining: remaining };
      });
    }, 1000);

    return () => {
      if (intervalRef.current) clearInterval(intervalRef.current);
    };
  }, [secret, generate]);

  return code;
}
