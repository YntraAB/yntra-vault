import { type YntraVaultBackend } from '@/lib/backend';
import type { Vault } from '@/types';
import { createContext,useContext } from 'react';


export interface AuthContextType {
  backend: YntraVaultBackend | null;
  backendReady: boolean;
  vaults: Vault[];
  currentVault: Vault | null;
  isLocked: boolean;
  setCurrentVault: (vault: Vault | null, replaceSession?: boolean) => void;
  setIsLocked: (locked: boolean) => void;
  lockVault: () => Promise<void>;
  addVault: (vault: Vault) => void;
  removeVault: (id: string) => void;
  getSessionGeneration: () => number;
  isSessionCurrent: (generation: number) => boolean;
}

export const AuthContext = createContext<AuthContextType | undefined>(undefined);


export function useAuth(): AuthContextType {
  const ctx = useContext(AuthContext);
  if (!ctx) throw new Error('useAuth must be used within AuthProvider');
  return ctx;
}
