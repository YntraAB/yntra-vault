import React, { createContext, useContext, useState, useCallback, useEffect, useMemo, useRef } from 'react';
import type { Vault } from '@/types';
import { isTauri, getBackend, type YntraVaultBackend } from '@/lib/backend';
import { clearSessionSecrets } from '@/lib/sessionSecrets';
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
  getSessionGeneration: () => number;
  isSessionCurrent: (generation: number) => boolean;
}

const AuthContext = createContext<AuthContextType | undefined>(undefined);

export function AuthProvider({ children }: { children: React.ReactNode }) {
  const [backend, setBackend] = useState<YntraVaultBackend | null>(null);
  const [backendReady, setBackendReady] = useState(false);
  const [vaults, setVaults] = useState<Vault[]>([]);
  const [currentVault, updateCurrentVault] = useState<Vault | null>(null);
  const [isLocked, updateIsLocked] = useState(true);
  const session = useRef({ generation: 0, locked: true, vault: null as Vault | null });
  const setCurrentVault = useCallback((vault: Vault | null) => {
    if (session.current.vault?.id !== vault?.id || session.current.vault?.path !== vault?.path) {
      session.current.generation++;
    }
    session.current.vault = vault;
    updateCurrentVault(vault);
  }, []);
  const setIsLocked = useCallback((locked: boolean) => {
    // Invalidate outstanding work synchronously, before React renders the lock screen.
    if (locked || session.current.locked !== locked) session.current.generation++;
    session.current.locked = locked;
    if (locked) {
      clearSessionSecrets();
    }
    window.dispatchEvent(new CustomEvent(locked ? 'yntra-session-locked' : 'yntra-session-unlocked'));
    updateIsLocked(locked);
  }, []);
  const getSessionGeneration = useCallback(() => session.current.generation, []);
  const isSessionCurrent = useCallback((generation: number) =>
    !session.current.locked && session.current.generation === generation, []);
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
            clearSessionSecrets();
            setIsLocked(true);
            setCurrentVault(null);
            addToast({
              message: 'Vault locked: the vault file or bound USB device is unavailable. Reconnect it and unlock again.',
              type: 'error',
            });
          });

          unsubLocked = await listen('vault-locked', () => {
            clearSessionSecrets();
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
  }, [addToast, setCurrentVault, setIsLocked]);

  const lockVault = useCallback(async () => {
    setIsLocked(true);
    clearSessionSecrets();
    if (backend) {
      try {
        await backend.lockVault();
      } catch (err) {
        console.error('Failed to lock backend vault:', err);
      }
    }
  }, [backend, setIsLocked]);

  useEffect(() => () => { session.current.generation++; session.current.locked = true; }, []);

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
      getSessionGeneration,
      isSessionCurrent,
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
      getSessionGeneration,
      isSessionCurrent,
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
