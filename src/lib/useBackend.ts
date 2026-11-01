/**
 * useBackend() — React hook for accessing the Yntra Vault backend
 * 
 * Provides lazy-initialized backend access with loading/error states.
 * Automatically detects Tauri vs WASM runtime.
 */

import { useState, useEffect, useCallback } from 'react';
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
      .catch((e) => setError((e instanceof Error ? e.message : String(e))))
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
    } catch (e) {
      setError(String(e));
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
    } catch (e) {
      setError(String(e));
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
    } catch (e) {
      setError(String(e));
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
    } catch (e) {
      setError(String(e));
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

  const getChallenge = useCallback(async (path: string) => {
    if (!backend) return null;
    try {
      return await backend.getHardware2FaChallenge(path);
    } catch {
      return null;
    }
  }, [backend]);

  const performChallenge = useCallback(async (protocol: Hardware2FaProtocol, challenge?: number[], credentialId?: number[]) => {
    if (!backend) return [];
    return backend.performHardware2FaChallenge(protocol, challenge, credentialId);
  }, [backend]);

  const enable = useCallback(async (
    password: string,
    keyFilePath: string | undefined,
    protocol: Hardware2FaProtocol,
    keyName: string,
    challengeSalt: number[] | undefined,
    credentialId: number[] | undefined,
    hardwareResponse: number[],
  ) => {
    if (!backend) return;
    setLoading(true);
    try {
      await backend.enableHardware2Fa(password, keyFilePath, protocol, keyName, challengeSalt, credentialId, hardwareResponse);
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

  return { info, loading, checkAvailability, isEnabled, getChallenge, performChallenge, enable, disable };
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
    } catch (e) {
      setError(String(e));
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
    } catch (e) {
      setError(String(e));
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
  const [result, setResult] = useState<{ id: string; entry: DecryptedEntry | null } | null>(null);
  useEffect(() => {
    if (!backend || !id) return;
    let active = true;
    const locked = () => { active = false; setResult(null); };
    window.addEventListener('yntra-session-locked', locked);
    void backend.getEntry(id).then(entry => { if (active) setResult({ id, entry }); })
      .catch(() => { if (active) setResult({ id, entry: null }); });
    return () => { active = false; window.removeEventListener('yntra-session-locked', locked); };
  }, [backend, id]);
  return { entry: id && result?.id === id ? result.entry : null, loading: Boolean(backend && id && result?.id !== id) };
}

export function useTotp(secret: string | null) {
  const { backend } = useBackend();
  const [result, setResult] = useState<{ secret: string; code: TotpCode | null } | null>(null);
  useEffect(() => {
    if (!backend || !secret) return;
    let active = true;
    let running = false;
    const locked = () => { active = false; setResult(null); };
    const generate = async () => {
      if (!active || running) return;
      running = true;
      try { const code = await backend.generateTotp(secret); if (active) setResult({ secret, code }); }
      catch { if (active) setResult({ secret, code: null }); }
      finally { running = false; }
    };
    window.addEventListener('yntra-session-locked', locked);
    void generate();
    const timer = setInterval(() => { void generate(); }, 1000);
    return () => { active = false; clearInterval(timer); window.removeEventListener('yntra-session-locked', locked); };
  }, [backend, secret]);
  return secret && result?.secret === secret ? result.code : null;
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



