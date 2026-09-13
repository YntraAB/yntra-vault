import { useState, useCallback } from 'react';
import { useBackend } from '@/lib/useBackend';

export function useBiometric() {
  const { backend } = useBackend();
  const [info, setInfo] = useState<{ available: boolean; biometric_type: string } | null>(null);
  const [loading, setLoading] = useState(false);

  const checkAvailability = useCallback(async () => {
    if (!backend) return null;
    try {
      const res = await backend.checkBiometricAvailable();
      setInfo(res);
      return res;
    } catch {
      return null;
    }
  }, [backend]);

  const isEnabled = useCallback(async (path: string) => {
    if (!backend) return false;
    try {
      return await backend.isBiometricEnabled(path);
    } catch {
      return false;
    }
  }, [backend]);

  const enable = useCallback(async () => {
    if (!backend) return;
    setLoading(true);
    try {
      await backend.enableBiometric();
    } finally {
      setLoading(false);
    }
  }, [backend]);

  const disable = useCallback(async () => {
    if (!backend) return;
    setLoading(true);
    try {
      await backend.disableBiometric();
    } finally {
      setLoading(false);
    }
  }, [backend]);

  return { info, loading, checkAvailability, isEnabled, enable, disable };
}
