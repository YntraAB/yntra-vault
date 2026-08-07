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
  Hardware2FaInfo,
  Hardware2FaProtocol,
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

// ─── Biometrics Hook ───────────────────────────────────────────────────

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

// ─── Hardware 2FA / YubiKey Hook ──────────────────────────────────────

export function useHardware2Fa() {
  const { backend } = useBackend();
  const [info, setInfo] = useState<Hardware2FaInfo | null>(null);
  const [loading, setLoading] = useState(false);

  const checkAvailability = useCallback(async () => {
    if (!backend) return null;
    try {
      const res = await backend.checkHardware2FaAvailable();
      setInfo(res);
      return res;
    } catch {
      return null;
    }
  }, [backend]);

  const isEnabled = useCallback(async (path: string) => {
    if (!backend) return false;
    try {
      return await backend.isHardware2FaEnabled(path);
    } catch {
      return false;
    }
  }, [backend]);

  const performChallenge = useCallback(async (protocol: Hardware2FaProtocol, challenge?: number[]) => {
    if (!backend) return [];
    return backend.performHardware2FaChallenge(protocol, challenge);
  }, [backend]);

  const enable = useCallback(async (protocol: Hardware2FaProtocol, keyName: string, hardwareResponse: number[]) => {
    if (!backend) return;
    setLoading(true);
    try {
      await backend.enableHardware2Fa(protocol, keyName, hardwareResponse);
    } finally {
      setLoading(false);
    }
  }, [backend]);

  const disable = useCallback(async () => {
    if (!backend) return;
    setLoading(true);
    try {
      await backend.disableHardware2Fa();
    } finally {
      setLoading(false);
    }
  }, [backend]);

  return { info, loading, checkAvailability, isEnabled, performChallenge, enable, disable };
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

  const runAudit = useCallback(async (disableSkeletonDelays = false, silent = false) => {
    if (!backend) return;
    if (!silent) setLoading(true);
    const startTime = Date.now();
    try {
      const result = await backend.securityAudit();
      const elapsed = Date.now() - startTime;
      if (!disableSkeletonDelays && !silent && elapsed < 250) {
        await new Promise(resolve => setTimeout(resolve, 250 - elapsed));
      }
      setAudit(result);
      return result;
    } finally {
      if (!silent) setLoading(false);
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



