import React, { createContext, useContext, useState, useCallback, useEffect, useMemo } from 'react';
import type { Vault } from '@/types';
import { isTauri, getBackend, type YntraVaultBackend } from '@/lib/backend';
import { clearTransientWebdavPassword } from '@/lib/sessionSecrets';
import { useToast } from '@/contexts/ToastContext';

export interface AuthContextType {
  backend: YntraVaultBackend | null;
  backendReady: boolean;
  vaults: Vault[];
  currentVault: Vault | null;
  isLocked: boolean;
  setCurrentVault: (vault: Vault | null) => void;
  setIsLocked: (locked: boolean) => void;
  lockVault: () => Promise<void>;
  addVault: (vault: Vault) => void;
  removeVault: (id: string) => void;
}

const AuthContext = createContext<AuthContextType | undefined>(undefined);

export function AuthProvider({ children }: { children: React.ReactNode }) {
  const [backend, setBackend] = useState<YntraVaultBackend | null>(null);
  const [backendReady, setBackendReady] = useState(false);
  const [vaults, setVaults] = useState<Vault[]>([]);
  const [currentVault, setCurrentVault] = useState<Vault | null>(null);
  const [isLocked, setIsLocked] = useState(false);
  const { addToast } = useToast();

  // Initialize backend when running in Tauri
  useEffect(() => {
    if (isTauri()) {
      getBackend()
        .then((b) => {
          setBackend(b);
          setBackendReady(true);
        })
        .catch((e) => {
          console.warn('Backend init failed, using mock data:', e);
          setBackendReady(true);
        });
    } else {
      // Browser dev mode — mock data
      setBackendReady(true);
    }
  }, []);

  // Listen for vault events in Tauri
  useEffect(() => {
    let unsubLost: (() => void) | null = null;
    let unsubLocked: (() => void) | null = null;

    if (isTauri()) {
      const initListen = async () => {
        try {
          const { listen } = await import('@tauri-apps/api/event');
          unsubLost = await listen('vault-connection-lost', () => {
            setIsLocked(true);
            setCurrentVault(null);
            addToast({
              message: '⚠️ ERROR: Vault database file was deleted, moved, or disconnected! Locked immediately.',
              type: 'error',
            });
          });

          unsubLocked = await listen('vault-locked', () => {
            setIsLocked(true);
          });
        } catch (e) {
          console.error('Failed to listen to tauri events:', e);
        }
      };
      initListen();
    }

    return () => {
      if (unsubLost) unsubLost();
      if (unsubLocked) unsubLocked();
    };
  }, [addToast]);

  const lockVault = useCallback(async () => {
    setIsLocked(true);
    clearTransientWebdavPassword();
    if (backend) {
      try {
        await backend.lockVault();
      } catch (err) {
        console.error('Failed to lock backend vault:', err);
      }
    }
  }, [backend]);

  const addVault = useCallback((vault: Vault) => {
    setVaults((prev) => [...prev, vault]);
  }, []);

  const removeVault = useCallback((id: string) => {
    setVaults((prev) => prev.filter((v) => v.id !== id));
  }, []);

  const value = useMemo(
    () => ({
      backend,
      backendReady,
      vaults,
      currentVault,
      isLocked,
      setCurrentVault,
      setIsLocked,
      lockVault,
      addVault,
      removeVault,
    }),
    [
      backend,
      backendReady,
      vaults,
      currentVault,
      isLocked,
      setCurrentVault,
      setIsLocked,
      lockVault,
      addVault,
      removeVault,
    ]
  );

  return (
    <AuthContext.Provider value={value}>
      {children}
    </AuthContext.Provider>
  );
}

export function useAuth(): AuthContextType {
  const ctx = useContext(AuthContext);
  if (!ctx) throw new Error('useAuth must be used within AuthProvider');
  return ctx;
}
