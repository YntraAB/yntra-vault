/**
 * useBackend() — React hook for accessing the Yntra Vault backend
 * 
 * Provides lazy-initialized backend access with loading/error states.
 * Automatically detects Tauri vs WASM runtime.
 */

import { useState, useEffect, useCallback, useRef } from 'react';
import { getBackend, type YntraVaultBackend } from './backend';
import type {
  VaultInfo,
  EntryPreview,
  DecryptedEntry,
  NewEntry,
  UpdateEntry,
  TotpCode,
  TotpConfig,
  GeneratorOptions,
  BreachResult,
  StrengthScore,
  SecurityAudit,
  Tag,
  TrashedEntryPreview,
  DecryptedHistoryItem,
} from './backend';

// ─── Core Backend Hook ──────────────────────────────────────────────────

export function useBackend() {
  const [backend, setBackend] = useState<YntraVaultBackend | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    getBackend()
      .then(setBackend)
      .catch((e) => setError(e.message))
      .finally(() => setLoading(false));
  }, []);

  return { backend, error, loading };
}

// ─── Vault Hook ─────────────────────────────────────────────────────────

export function useVault() {
  const { backend } = useBackend();
  const [vaultInfo, setVaultInfo] = useState<VaultInfo | null>(null);
  const [isLocked, setIsLocked] = useState(true);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const createVault = useCallback(async (name: string, password: string, path: string) => {
    if (!backend) return;
    setLoading(true);
    setError(null);
    try {
      const info = await backend.createVault(name, password, path);
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

  const openVault = useCallback(async (path: string, password: string) => {
    if (!backend) return;
    setLoading(true);
    setError(null);
    try {
      const info = await backend.openVault(path, password);
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
    await backend.lockVault();
    setVaultInfo(null);
    setIsLocked(true);
  }, [backend]);

  return { vaultInfo, isLocked, loading, error, createVault, openVault, lockVault };
}

// ─── Entries Hook ───────────────────────────────────────────────────────

export function useEntries() {
  const { backend } = useBackend();
  const [entries, setEntries] = useState<EntryPreview[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    if (!backend) return;
    setLoading(true);
    try {
      const list = await backend.listEntries();
      setEntries(list);
    } catch (e: any) {
      setError(e.toString());
    } finally {
      setLoading(false);
    }
  }, [backend]);

  const search = useCallback(async (query: string) => {
    if (!backend) return;
    setLoading(true);
    try {
      const results = await backend.searchEntries(query);
      setEntries(results);
    } catch (e: any) {
      setError(e.toString());
    } finally {
      setLoading(false);
    }
  }, [backend]);

  const addEntry = useCallback(async (entry: NewEntry) => {
    if (!backend) return;
    const id = await backend.addEntry(entry);
    await refresh();
    return id;
  }, [backend, refresh]);

  const updateEntry = useCallback(async (id: string, update: UpdateEntry) => {
    if (!backend) return;
    await backend.updateEntry(id, update);
    await refresh();
  }, [backend, refresh]);

  const deleteEntry = useCallback(async (id: string) => {
    if (!backend) return;
    await backend.deleteEntry(id);
    await refresh();
  }, [backend, refresh]);

  const toggleFavorite = useCallback(async (id: string) => {
    if (!backend) return;
    const newState = await backend.toggleFavorite(id);
    await refresh();
    return newState;
  }, [backend, refresh]);

  return { entries, loading, error, refresh, search, addEntry, updateEntry, deleteEntry, toggleFavorite };
}

// ─── Single Entry Hook ──────────────────────────────────────────────────

export function useEntry(id: string | null) {
  const { backend } = useBackend();
  const [entry, setEntry] = useState<DecryptedEntry | null>(null);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (!backend || !id) {
      setEntry(null);
      return;
    }
    setLoading(true);
    backend.getEntry(id)
      .then(setEntry)
      .catch(() => setEntry(null))
      .finally(() => setLoading(false));
  }, [backend, id]);

  return { entry, loading };
}

// ─── TOTP Hook (auto-refreshing) ────────────────────────────────────────

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

    // Refresh every second for countdown, regenerate when period expires
    intervalRef.current = setInterval(() => {
      setCode((prev) => {
        if (!prev) return null;
        const remaining = prev.seconds_remaining - 1;
        if (remaining <= 0) {
          generate(); // Get fresh code
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

// ─── Password Generator Hook ────────────────────────────────────────────

export function usePasswordGenerator() {
  const { backend } = useBackend();
  const [password, setPassword] = useState<string>('');
  const [strength, setStrength] = useState<StrengthScore | null>(null);
  const [breach, setBreach] = useState<BreachResult | null>(null);

  const generate = useCallback(async (options?: GeneratorOptions) => {
    if (!backend) return '';
    let pw = '';
    let isBreached = true;
    let attempts = 0;

    // Generate and check against HIBP database. Loop up to 5 times.
    while (isBreached && attempts < 5) {
      attempts++;
      pw = options
        ? await backend.generatePassword(options)
        : await backend.generatePasswordDefault();
      
      try {
        const result = await backend.checkPasswordBreach(pw);
        isBreached = result.is_breached;
      } catch {
        // Safe fallback in case of no network / API errors
        isBreached = false;
      }
    }

    setPassword(pw);

    // Auto-analyze strength
    const score = await backend.analyzePasswordStrength(pw);
    setStrength(score);

    return pw;
  }, [backend]);

  const checkBreach = useCallback(async (pw?: string) => {
    if (!backend) return;
    const target = pw || password;
    if (!target) return;
    try {
      const result = await backend.checkPasswordBreach(target);
      setBreach(result);
      return result;
    } catch {
      // Offline — skip breach check
    }
  }, [backend, password]);

  const analyzeStrength = useCallback(async (pw: string) => {
    if (!backend) return null;
    const score = await backend.analyzePasswordStrength(pw);
    setStrength(score);
    return score;
  }, [backend]);

  return { password, strength, breach, generate, checkBreach, analyzeStrength };
}

// ─── Security Audit Hook ────────────────────────────────────────────────

export function useSecurityAudit() {
  const { backend } = useBackend();
  const [audit, setAudit] = useState<SecurityAudit | null>(null);
  const [loading, setLoading] = useState(false);

  const runAudit = useCallback(async (disableSkeletonDelays = false) => {
    if (!backend) return;
    setLoading(true);
    const startTime = Date.now();
    try {
      const result = await backend.securityAudit();
      const elapsed = Date.now() - startTime;
      if (!disableSkeletonDelays && elapsed < 250) {
        await new Promise(resolve => setTimeout(resolve, 250 - elapsed));
      }
      setAudit(result);
      return result;
    } finally {
      setLoading(false);
    }
  }, [backend]);

  return { audit, loading, runAudit };
}

// Re-export types for convenience
export type {
  VaultInfo,
  EntryPreview,
  DecryptedEntry,
  NewEntry,
  UpdateEntry,
  TotpCode,
  TotpConfig,
  GeneratorOptions,
  BreachResult,
  StrengthScore,
  SecurityAudit,
  Tag,
  TrashedEntryPreview,
  DecryptedHistoryItem,
};



