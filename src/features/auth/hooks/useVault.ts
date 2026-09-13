import { useState, useCallback } from 'react';
import { useBackend } from '@/lib/useBackend';
import type { VaultInfo } from '@/lib/backend';

export function useVault() {
  const { backend } = useBackend();
  const [vaultInfo, setVaultInfo] = useState<VaultInfo | null>(null);
  const [isLocked, setIsLocked] = useState(true);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const createVault = useCallback(async (name: string, password: string, path: string, keyFilePath?: string) => {
    if (!backend) return;
    setLoading(true);
    setError(null);
    try {
      const info = await backend.createVault(name, password, path, keyFilePath);
      setVaultInfo(info);
      setIsLocked(false);
      return info;
    } catch (e: any) {
      setError(e.toString());
      throw e;
    } finally {
      setLoading(false);
    }
  }, [backend]);

  const openVault = useCallback(async (path: string, password: string, keyFilePath?: string) => {
    if (!backend) return;
    setLoading(true);
    setError(null);
    try {
      const info = await backend.openVault(path, password, keyFilePath);
      setVaultInfo(info);
      setIsLocked(false);
      return info;
    } catch (e: any) {
      setError(e.toString());
      throw e;
    } finally {
      setLoading(false);
    }
  }, [backend]);

  const unlockVaultBiometric = useCallback(async (path: string) => {
    if (!backend) return;
    setLoading(true);
    setError(null);
    try {
      const info = await backend.unlockVaultBiometric(path);
      setVaultInfo(info);
      setIsLocked(false);
      return info;
    } catch (e: any) {
      setError(e.toString());
      throw e;
    } finally {
      setLoading(false);
    }
  }, [backend]);

  const lockVault = useCallback(async () => {
    if (!backend) return;
    setLoading(true);
    setError(null);
    try {
      await backend.lockVault();
      setVaultInfo(null);
      setIsLocked(true);
    } catch (e: any) {
      setError(e.toString());
      throw e;
    } finally {
      setLoading(false);
    }
  }, [backend]);

  return { vaultInfo, isLocked, loading, error, createVault, openVault, unlockVaultBiometric, lockVault };
}
