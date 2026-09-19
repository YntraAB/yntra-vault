import { useState, useCallback, useRef, useEffect } from 'react';
import { motion } from 'framer-motion';
import { Eye, EyeOff, Loader2, AlertTriangle, KeyRound, FolderOpen, ShieldCheck, Copy, Fingerprint } from 'lucide-react';
import { useNavigate } from 'react-router-dom';
import { useAuth } from '@/features/auth';
import { useTranslation } from '@/contexts/LanguageContext';
import { isTauri, getBackend, openFileDialog } from '@/lib/backend';
import { ActionTooltip } from '@/components/ui/tooltip';
import { SecureSecretInput, type SecureSecretInputRef } from '@/components/ui';

const MAX_ATTEMPTS = 5;
const LOCKOUT_DELAYS = [0, 0, 0, 5000, 15000, 30000]; // ms delay per attempt

export default function Login() {
  const navigate = useNavigate();
  const { t } = useTranslation();
  const { currentVault, setIsLocked, setCurrentVault } = useAuth();
  const [password, setPassword] = useState('');
  const [showPassword, setShowPassword] = useState(false);
  const [useKeyFile, setUseKeyFile] = useState(false);
  const [keyFilePath, setKeyFilePath] = useState('');
  const [error, setError] = useState('');
  const [shake, setShake] = useState(false);
  const [loading, setLoading] = useState(false);
  const [attempts, setAttempts] = useState(0);
  const [lockedUntil, setLockedUntil] = useState(0);
  const [biometricAvailable, setBiometricAvailable] = useState(false);
  const [biometricType, setBiometricType] = useState('Windows Hello');
  const [biometricPrompting, setBiometricPrompting] = useState(false);
  const [hardware2FaRequired, setHardware2FaRequired] = useState(false);
  const [activeView, setActiveView] = useState<'master_password' | 'biometric' | 'hardware_2fa' | 'emergency_recovery'>('master_password');
  const [recoveryShareA, setRecoveryShareA] = useState('');
  const [recoveryShareB, setRecoveryShareB] = useState('');
  const [recoveredPassword, setRecoveredPassword] = useState('');
  const [recoveryError, setRecoveryError] = useState('');
  const [recovering, setRecovering] = useState(false);
  const [copiedRecovered, setCopiedRecovered] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);
  const secretInputRef = useRef<SecureSecretInputRef>(null);
  const autoBioTriggered = useRef(false);

  const isLockedOut = Date.now() < lockedUntil;
  const lockoutRemaining = Math.ceil((lockedUntil - Date.now()) / 1000);

  const triggerShake = useCallback(() => {
    setShake(true);
    setTimeout(() => setShake(false), 300);
  }, []);

  // Redirect if not in Tauri desktop mode or no vault is selected & check biometrics / Hardware 2FA
  useEffect(() => {
    if (!isTauri()) {
      navigate('/');
      return;
    }

    if (!currentVault) {
      getBackend().then(async (backend) => {
        try {
          const saved = JSON.parse(localStorage.getItem('yntra-vault-recent-vaults') || '[]');
          for (const v of saved) {
            try {
              if (await backend.checkVaultFileExists(v.path)) {
                setCurrentVault(v);
                return;
              }
            } catch {}
          }
          navigate('/');
        } catch {
          navigate('/');
        }
      });
      return;
    }

    if (currentVault?.path) {
      // Security: Do not auto-persist or auto-load keyfile paths to protect 2FA factor isolation.
      // Purge any legacy stored paths.
      try {
        localStorage.removeItem('yntra-vault-keyfiles');
      } catch {}

      getBackend().then(async (backend) => {
        try {
          const hwEnabled = await backend.isHardware2FaEnabled(currentVault.path);
          setHardware2FaRequired(hwEnabled);

          const enabled = await backend.isBiometricEnabled(currentVault.path);
          const info = await backend.checkBiometricAvailable();
          const isBio = enabled && info.available;
          setBiometricAvailable(isBio);
          if (info.biometric_type) {
            setBiometricType(info.biometric_type);
          }

          if (hwEnabled) {
            setActiveView('hardware_2fa');
          } else if (isBio) {
            setActiveView('biometric');
          } else {
            setActiveView('master_password');
          }
        } catch (e) {
          console.error('Security check error:', e);
        }
      });
    }
  }, [currentVault, navigate, setCurrentVault]);

  const handleBrowseKeyFile = async () => {
    if (!isTauri()) return;
    try {
      const selected = await openFileDialog({
        title: 'Select Key File',
        multiple: false,
        filters: [{ name: 'Key File (*.key, *.*)', extensions: ['key', '*'] }],
      });
      if (selected) {
        setKeyFilePath(typeof selected === 'string' ? selected : String(selected[0]));
      }
    } catch (e) {
      console.error('Key file selection failed:', e);
    }
  };

  const handleUnlockBiometric = useCallback(async () => {
    if (!currentVault?.path || !isTauri()) return;
    if (typeof document !== 'undefined' && (document.hidden || document.visibilityState !== 'visible')) {
      autoBioTriggered.current = false;
      return;
    }
    setError('');
    setBiometricPrompting(true);
    setLoading(true);

    try {
      const backend = await getBackend();
      const info = await backend.unlockVaultBiometric(currentVault.path);

      // Save to recent vaults list & update search paths using internal ID
      const recent = JSON.parse(localStorage.getItem('yntra-vault-recent-vaults') || '[]');
      const updated = recent.filter((v: any) => v.id !== info.id && v.path !== info.path);
      const newVault = { id: info.id, name: info.name, path: info.path };
      localStorage.setItem('yntra-vault-recent-vaults', JSON.stringify([newVault, ...updated.slice(0, 9)]));

      // Update currentVault state in global context with real ID & path
      setCurrentVault(newVault);

      // Success
      setIsLocked(false);
      setAttempts(0);
      navigate('/app');
    } catch (err: any) {
      const msg = err?.message || String(err) || '';
      console.warn('Biometric unlock attempt:', msg);

      if (msg.includes('Window is hidden') || msg.includes('Window is not visible')) {
        // Silently reset without error banner when window was closed/hidden; allow restore to trigger prompt
        autoBioTriggered.current = false;
        return;
      }

      if (msg.toLowerCase().includes('canceled') || msg.toLowerCase().includes('cancelled')) {
        setError(t('login.biometric_canceled') || 'Biometric prompt was dismissed');
      } else if (msg.includes('Hardware2FaRequired')) {
        setHardware2FaRequired(true);
        setActiveView('hardware_2fa');
      } else {
        setError(msg || t('login.biometric_failed') || 'Biometric verification failed');
        triggerShake();
      }
    } finally {
      setBiometricPrompting(false);
      setLoading(false);
    }
  }, [currentVault, navigate, setCurrentVault, setIsLocked, t, triggerShake]);

  useEffect(() => {
    // Only auto-trigger when window and document are actively visible
    const isDocVisible = typeof document === 'undefined' || (!document.hidden && document.visibilityState === 'visible');

    if (activeView === 'biometric' && biometricAvailable && !autoBioTriggered.current && !loading && isDocVisible) {
      autoBioTriggered.current = true;
      const timer = setTimeout(() => {
        handleUnlockBiometric();
      }, 150);
      return () => clearTimeout(timer);
    }

    // When the app is restored/focused from tray, trigger biometrics if still locked & not yet triggered
    const handleVisibility = () => {
      if (typeof document !== 'undefined' && !document.hidden && document.visibilityState === 'visible') {
        if (activeView === 'biometric' && biometricAvailable && !autoBioTriggered.current && !loading) {
          autoBioTriggered.current = true;
          handleUnlockBiometric();
        }
      }
    };

    document.addEventListener('visibilitychange', handleVisibility);
    window.addEventListener('focus', handleVisibility);

    return () => {
      document.removeEventListener('visibilitychange', handleVisibility);
      window.removeEventListener('focus', handleVisibility);
    };
  }, [activeView, biometricAvailable, handleUnlockBiometric, loading]);

  const handleHardwareUnlock = useCallback(async (e?: React.FormEvent) => {
    if (e) e.preventDefault();
    setError('');
    if (isLockedOut) {
      setError(t('login.err_locked', { seconds: lockoutRemaining }) || `Too many attempts. Try again in ${lockoutRemaining}s`);
      return;
    }

    if (!password) {
      setError(t('login.err_enter_password') || 'Please enter your master password');
      inputRef.current?.focus();
      return;
    }

    setLoading(true);
    try {
      if (isTauri() && currentVault) {
        const backend = await getBackend();
        const kf = useKeyFile && keyFilePath.trim() ? keyFilePath.trim() : undefined;

        // Retrieve persistent challenge salt and protocol from vault file header
        const challInfo = await backend.getHardware2FaChallenge(currentVault.path);
        if (!challInfo || !challInfo.enabled) {
          throw new Error('Hardware 2FA header not found in vault file');
        }

        const hwResp = await backend.performHardware2FaChallenge(
          challInfo.protocol,
          challInfo.challenge_salt,
          challInfo.credential_id?.length ? challInfo.credential_id : undefined,
        );

        const info = await backend.openVaultWithHardware2Fa(currentVault.path, password, kf, hwResp);

        // Save to recent vaults list & update search paths using internal ID
        const recent = JSON.parse(localStorage.getItem('yntra-vault-recent-vaults') || '[]');
        const updated = recent.filter((v: any) => v.id !== info.id && v.path !== info.path);
        const newVault = { id: info.id, name: info.name, path: info.path };
        localStorage.setItem('yntra-vault-recent-vaults', JSON.stringify([newVault, ...updated.slice(0, 9)]));

        setCurrentVault(newVault);
        setIsLocked(false);
        setPassword('');
        setAttempts(0);
        navigate('/app');
      }
    } catch (err: any) {
      console.error('Hardware unlock error:', err);
      const nextAttempts = attempts + 1;
      setAttempts(nextAttempts);

      if (nextAttempts >= MAX_ATTEMPTS) {
        const delay = LOCKOUT_DELAYS[Math.min(nextAttempts, LOCKOUT_DELAYS.length - 1)];
        setLockedUntil(Date.now() + delay);
        setError(t('login.err_locked', { seconds: delay / 1000 }) || `Too many failed attempts. Locked for ${delay / 1000}s`);
      } else {
        const errMsg = err?.toString() || 'Hardware key authentication failed';
        if (errMsg.includes('InvalidPassword')) {
          setError(t('error.invalid_password') || 'Incorrect master password');
        } else if (errMsg.includes('Hardware2FaAuthFailed')) {
          setError(t('error.hardware_2fa_auth_failed') || 'Security key authentication failed. Touch rejected or timed out.');
        } else {
          setError(errMsg);
        }
      }
      triggerShake();
    } finally {
      setLoading(false);
    }
  }, [password, isLockedOut, lockoutRemaining, currentVault, useKeyFile, keyFilePath, attempts, triggerShake, setCurrentVault, setIsLocked, navigate, t]);

  const handleSubmit = useCallback(
    async (e: React.FormEvent) => {
      e.preventDefault();
      setError('');

      if (isLockedOut) {
        setError(t('login.err_locked', { seconds: lockoutRemaining }) || `Too many attempts. Try again in ${lockoutRemaining}s`);
        return;
      }

      const secretBytes = secretInputRef.current?.getSecretBytes();
      const passBytes = secretBytes && secretBytes.length > 0 ? secretBytes : new TextEncoder().encode(password);

      if (passBytes.length === 0) {
        setError(t('login.err_enter_password') || 'Enter your master password');
        triggerShake();
        return;
      }

      if (useKeyFile && !keyFilePath.trim()) {
        setError(t('login.err_specify_keyfile') || 'Please select or specify a Key File');
        triggerShake();
        return;
      }

      if (hardware2FaRequired) {
        return handleHardwareUnlock(e);
      }

      setLoading(true);
      try {
        let info;
        if (isTauri() && currentVault) {
          const backend = await getBackend();
          const kf = useKeyFile && keyFilePath.trim() ? keyFilePath.trim() : undefined;
          info = await backend.openVaultBytes(currentVault.path, passBytes, kf);
        } else {
          info = { id: currentVault?.id || crypto.randomUUID(), name: currentVault?.name || 'Vault', path: currentVault?.path || '' };
        }

        // Save to recent vaults list & update search paths using internal ID
        const recent = JSON.parse(localStorage.getItem('yntra-vault-recent-vaults') || '[]');
        const updated = recent.filter((v: any) => v.id !== info.id && v.path !== info.path);
        const newVault = { id: info.id, name: info.name, path: info.path };
        localStorage.setItem('yntra-vault-recent-vaults', JSON.stringify([newVault, ...updated.slice(0, 9)]));

        // Update currentVault state in global context with real ID & path
        setCurrentVault(newVault);

        // Success
        setIsLocked(false);
        setPassword(''); // Security: clear from state
        setAttempts(0);
        navigate('/app');
      } catch (err: any) {
        const errMsg = err.toString();
        if (errMsg.includes('Hardware 2FA / YubiKey required') || errMsg.includes('Hardware2FaRequired')) {
          setHardware2FaRequired(true);
          setActiveView('hardware_2fa');
          setError(t('error.hardware_2fa_required') || 'Security key required. Touch your hardware key to unlock.');
          triggerShake();
        } else {
          const newAttempts = attempts + 1;
          setAttempts(newAttempts);

          // Rate limiting
          const delay = LOCKOUT_DELAYS[Math.min(newAttempts, LOCKOUT_DELAYS.length - 1)];
          if (delay > 0) {
            setLockedUntil(Date.now() + delay);
            setError(t('login.err_locked', { seconds: delay / 1000 }) || `Incorrect password or key file. Locked for ${delay / 1000}s`);

            // Auto-unlock countdown
            setTimeout(() => {
              setLockedUntil(0);
              setError('');
              inputRef.current?.focus();
            }, delay);
          } else {
            setError(t('login.err_incorrect') || 'Incorrect password or invalid key file');
          }

          triggerShake();
          setPassword('');
        }
      } finally {
        passBytes.fill(0);
        secretInputRef.current?.clearSecretBytes();
        setLoading(false);
      }
    },
    [password, useKeyFile, keyFilePath, hardware2FaRequired, handleHardwareUnlock, setIsLocked, setCurrentVault, navigate, currentVault, attempts, isLockedOut, lockoutRemaining, t, triggerShake]
  );

  const handleEmergencyRecovery = useCallback(async (e: React.FormEvent) => {
    e.preventDefault();
    setRecoveryError('');

    const sA = recoveryShareA.trim();
    const sB = recoveryShareB.trim();

    if (!sA || !sB) {
      setRecoveryError('Please provide both recovery shares');
      return;
    }

    if (sA === sB) {
      setRecoveryError('Cannot reconstruct using two identical shares');
      return;
    }

    setRecovering(true);
    try {
      const backend = await getBackend();
      const reconstructed = await backend.reconstructMasterPassword(sA, sB);
      setRecoveredPassword(reconstructed);
    } catch (err: unknown) {
      setRecoveryError((err as Error)?.message || String(err) || 'Failed to reconstruct password from shares');
    } finally {
      setRecovering(false);
    }
  }, [recoveryShareA, recoveryShareB]);

  const handleUnlockWithRecovered = useCallback(async () => {
    if (!recoveredPassword || !currentVault) return;
    setLoading(true);
    setRecoveryError('');
    const passBytes = new TextEncoder().encode(recoveredPassword);
    try {
      let info;
      if (isTauri() && currentVault) {
        const backend = await getBackend();
        const kf = useKeyFile && keyFilePath.trim() ? keyFilePath.trim() : undefined;
        info = await backend.openVaultBytes(currentVault.path, passBytes, kf);
      } else {
        info = { id: currentVault?.id || crypto.randomUUID(), name: currentVault?.name || 'Vault', path: currentVault?.path || '' };
      }

      const recent = JSON.parse(localStorage.getItem('yntra-vault-recent-vaults') || '[]');
      const updated = recent.filter((v: { id?: string; path?: string }) => v.id !== info.id && v.path !== info.path);
      const newVault = { id: info.id, name: info.name, path: info.path };
      localStorage.setItem('yntra-vault-recent-vaults', JSON.stringify([newVault, ...updated.slice(0, 9)]));

      setCurrentVault(newVault);
      setIsLocked(false);
      setRecoveredPassword('');
      navigate('/app');
    } catch (err: unknown) {
      setRecoveryError((err as Error)?.message || String(err) || 'Failed to unlock vault with recovered password');
    } finally {
      passBytes.fill(0);
      setLoading(false);
    }
  }, [recoveredPassword, currentVault, useKeyFile, keyFilePath, setCurrentVault, setIsLocked, navigate]);

  return (
    <motion.div
      initial={{ opacity: 0, y: 8 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ duration: 0.2, ease: 'easeOut' }}
      className="flex min-h-dvh w-full justify-center bg-[var(--bg-base)] px-4 pt-[env(safe-area-inset-top,0px)] pb-[env(safe-area-inset-bottom,0px)] overflow-y-auto touch-pan-y overscroll-contain"
    >
      <div className="w-full max-w-[380px] py-6 my-auto">
        {/* Header */}
        <div className="flex flex-col items-center text-center">
          <img
            src="/white-logo.png"
            alt="Yntra Vault"
            className="mb-3 h-20 w-20 rounded-[3px] object-cover invert dark:invert-0"
          />
          <h1 className="text-[18px] font-semibold tracking-tight text-[var(--text-primary)]">
            {currentVault?.name || 'Vault'}
          </h1>
          {hardware2FaRequired && (
            <div className="mt-2 inline-flex items-center gap-1.5 rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] px-2.5 py-1 text-[11px] font-medium text-[var(--text-primary)]">
              <KeyRound size={12} />
              <span>{t('login.hardware_key_enrolled') || 'Hardware Key Enrolled'}</span>
            </div>
          )}
          <p className="mt-1 text-[13px] text-[var(--text-secondary)]">
            {activeView === 'biometric'
              ? (t('login.biometric_desc', { bioType: biometricType.split('(')[0].trim() }) || `Use ${biometricType.split('(')[0].trim()} to unlock your vault`)
              : activeView === 'hardware_2fa'
              ? (t('login.hardware_key_desc') || 'Touch your security key to unlock')
              : activeView === 'emergency_recovery'
              ? t('login.emergency_recovery')
              : t('login.enter_master')}
          </p>
        </div>

        {/* Views depending on vault capabilities */}
        {activeView === 'hardware_2fa' ? (
          <form onSubmit={handleHardwareUnlock} className="mt-6 flex flex-col gap-4">
            <motion.div
              animate={shake ? { x: [0, -4, 4, -4, 4, 0] } : {}}
              transition={{ duration: 0.3 }}
              className="flex flex-col items-center rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] p-5 text-center shadow-sm"
            >
              <div className="mb-3 flex h-12 w-12 items-center justify-center rounded-full border border-[var(--border)] bg-[var(--accent-bg)] text-[var(--text-primary)]">
                <ShieldCheck size={24} />
              </div>
              <h2 className="text-[15px] font-semibold tracking-tight text-[var(--text-primary)]">
                {t('login.hardware_key_title') || 'Two-Factor Authentication'}
              </h2>
              <p className="mt-1 text-[12px] text-[var(--text-secondary)]">
                {t('login.hardware_key_touch') || 'Enter your master password and touch your security key'}
              </p>

              {error && (
                <div className="mt-3 flex w-full items-center gap-1.5 rounded-[3px] bg-[var(--destructive)]/10 px-3 py-1.5 text-[12px] text-[var(--destructive)]">
                  <AlertTriangle size={13} className="shrink-0" />
                  <span className="select-text">{error}</span>
                </div>
              )}
            </motion.div>

            {/* Master Password Input (Factor 1) */}
            <div className="flex flex-col gap-1.5">
              <label className="text-[12px] font-medium text-[var(--text-secondary)]">
                {t('login.master_password') || 'Master Password'}
              </label>
              <div className="relative">
                <input
                  ref={inputRef}
                  type={showPassword ? 'text' : 'password'}
                  value={password}
                  onChange={(e) => {
                    setPassword(e.target.value);
                    if (error) setError('');
                  }}
                  placeholder={t('login.password_placeholder') || 'Enter master password'}
                  disabled={loading || isLockedOut}
                  autoFocus
                  className="h-10 w-full rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] pl-3 pr-10 text-[13px] text-[var(--text-primary)] placeholder-[var(--text-tertiary)] outline-none transition-colors focus:border-[var(--border-focus)] disabled:opacity-50"
                />
                <button
                  type="button"
                  onClick={() => setShowPassword(!showPassword)}
                  className="absolute right-3 top-1/2 -translate-y-1/2 text-[var(--text-tertiary)] hover:text-[var(--text-primary)]"
                >
                  {showPassword ? <EyeOff size={15} /> : <Eye size={15} />}
                </button>
              </div>
            </div>

            {/* Key File Option */}
            <div className="flex flex-col gap-1.5">
              <div className="flex items-center justify-between">
                <label className="flex items-center gap-2 cursor-pointer select-none text-[12px] text-[var(--text-secondary)] hover:text-[var(--text-primary)]">
                  <input
                    type="checkbox"
                    checked={useKeyFile}
                    onChange={(e) => setUseKeyFile(e.target.checked)}
                    className="rounded border-[var(--border)] text-[var(--text-primary)] focus:ring-0"
                  />
                  <span>{t('login.use_key_file') || 'Use Key File'}</span>
                </label>
              </div>
              {useKeyFile && (
                <div className="flex gap-2">
                  <input
                    type="text"
                    value={keyFilePath}
                    onChange={(e) => setKeyFilePath(e.target.value)}
                    placeholder={t('login.key_file_path') || 'Path to .key file'}
                    className="h-8 flex-1 rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] px-2.5 text-[12px] text-[var(--text-primary)] placeholder-[var(--text-tertiary)] outline-none focus:border-[var(--border-focus)]"
                  />
                  <button
                    type="button"
                    onClick={handleBrowseKeyFile}
                    className="h-8 rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] px-3 text-[12px] font-medium text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)] transition-colors"
                  >
                    {t('common.browse') || 'Browse'}
                  </button>
                </div>
              )}
            </div>

            {/* Submit Button */}
            <button
              type="submit"
              disabled={loading || !password || isLockedOut}
              className="flex h-10 w-full items-center justify-center gap-2 rounded-[3px] bg-[var(--accent)] text-[13px] font-semibold text-[var(--bg-base)] transition-colors hover:bg-[var(--accent-hover)] disabled:opacity-50 cursor-pointer disabled:cursor-not-allowed"
            >
              {loading ? (
                <>
                  <Loader2 size={14} className="animate-spin" />
                  <span>{t('login.authenticating_key') || 'Awaiting Key Touch...'}</span>
                </>
              ) : (
                <>
                  <ShieldCheck size={16} />
                  <span>{t('login.touch_key_btn') || 'Unlock with Security Key'}</span>
                </>
              )}
            </button>

            {/* Emergency Recovery */}
            <button
              type="button"
              onClick={() => {
                setError('');
                setActiveView('emergency_recovery');
              }}
              className="mt-1 text-center text-[12px] text-[var(--text-tertiary)] hover:text-[var(--text-secondary)] transition-colors cursor-pointer"
            >
              {t('login.emergency_recovery_btn') || 'Lost security key? Use emergency recovery shares'}
            </button>
          </form>
        ) : activeView === 'biometric' ? (
          <div className="mt-6 flex flex-col gap-3">
            <motion.div
              animate={shake ? { x: [0, -4, 4, -4, 4, 0] } : {}}
              transition={{ duration: 0.3 }}
              className="flex flex-col items-center rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] p-5 text-center shadow-sm"
            >
              <div className="relative mb-3 flex h-14 w-14 items-center justify-center rounded-full border border-[var(--accent)]/30 bg-[var(--accent)]/10 text-[var(--accent)]">
                {biometricPrompting ? (
                  <Loader2 size={26} className="animate-spin text-[var(--accent)]" />
                ) : (
                  <Fingerprint size={28} />
                )}
              </div>
              <h2 className="text-[15px] font-semibold tracking-tight text-[var(--text-primary)]">
                {t('login.biometric_title') || 'Biometric Unlock'}
              </h2>
              <p className="mt-1 text-[12px] text-[var(--text-secondary)]">
                {biometricType}
              </p>

              {error && (
                <div className="mt-3 flex items-center gap-1.5 rounded-[3px] bg-[var(--destructive)]/10 px-3 py-1.5 text-[12px] text-[var(--destructive)]">
                  <AlertTriangle size={13} className="shrink-0" />
                  <span>{error}</span>
                </div>
              )}
            </motion.div>

            {/* Unlock with Biometric button */}
            <button
              type="button"
              onClick={handleUnlockBiometric}
              disabled={loading}
              className="flex h-10 w-full items-center justify-center gap-2 rounded-[3px] bg-[var(--text-primary)] text-[13px] font-semibold text-[var(--bg-base)] transition-all hover:opacity-90 active:scale-[0.99] disabled:opacity-50 cursor-pointer disabled:cursor-not-allowed"
            >
              {loading ? (
                <>
                  <Loader2 size={14} className="animate-spin" />
                  <span>{t('login.biometric_unlocking') || 'Awaiting verification...'}</span>
                </>
              ) : (
                <>
                  <Fingerprint size={16} />
                  <span>
                    {error
                      ? (t('login.biometric_retry') || 'Try Again')
                      : (t('login.biometric_btn', { bioType: biometricType.split('(')[0].trim() }) || `Unlock with ${biometricType.split('(')[0].trim()}`)}
                  </span>
                </>
              )}
            </button>

            {/* Master Password Fallback */}
            <button
              type="button"
              onClick={() => {
                setError('');
                setActiveView('master_password');
              }}
              disabled={loading}
              className="flex h-9 w-full items-center justify-center gap-1.5 rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] text-[12px] font-medium text-[var(--text-secondary)] transition-colors hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)] cursor-pointer disabled:opacity-50"
            >
              <KeyRound size={13} />
              <span>{t('login.use_master_password') || 'Use Master Password'}</span>
            </button>
          </div>
        ) : activeView === 'emergency_recovery' ? (
          <div className="mt-6 flex flex-col gap-3">
            <div className="flex flex-col items-center rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] p-4 text-center shadow-sm">
              <div className="mb-2.5 flex h-9 w-9 items-center justify-center rounded-[3px] border border-[var(--border)] bg-[var(--bg-surface)] text-[var(--text-secondary)]">
                <KeyRound size={16} />
              </div>
              <h2 className="text-[14px] font-semibold tracking-tight text-[var(--text-primary)]">
                {t('login.emergency_recovery')}
              </h2>
              <p className="mt-1 text-[11px] text-[var(--text-secondary)]">
                {t('login.emergency_recovery_desc')}
              </p>

              {recoveryError && (
                <div className="mt-3 flex w-full items-center gap-1.5 rounded-[3px] bg-[var(--destructive)]/10 px-3 py-1.5 text-left text-[11px] text-[var(--destructive)]">
                  <AlertTriangle size={13} className="shrink-0" />
                  <span className="select-text">{recoveryError}</span>
                </div>
              )}

              {!recoveredPassword ? (
                <form onSubmit={handleEmergencyRecovery} className="mt-3 flex w-full flex-col gap-2">
                  <input
                    type="text"
                    value={recoveryShareA}
                    onChange={(e) => {
                      setRecoveryShareA(e.target.value);
                      setRecoveryError('');
                    }}
                    placeholder={t('settings.share1_placeholder')}
                    className="h-8.5 w-full rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] px-2.5 font-mono text-[11px] text-[var(--text-primary)] outline-none focus:border-[var(--border-focus)] placeholder:font-sans placeholder:text-[var(--text-tertiary)]"
                  />
                  <input
                    type="text"
                    value={recoveryShareB}
                    onChange={(e) => {
                      setRecoveryShareB(e.target.value);
                      setRecoveryError('');
                    }}
                    placeholder={t('settings.share2_placeholder')}
                    className="h-8.5 w-full rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] px-2.5 font-mono text-[11px] text-[var(--text-primary)] outline-none focus:border-[var(--border-focus)] placeholder:font-sans placeholder:text-[var(--text-tertiary)]"
                  />
                  <button
                    type="submit"
                    disabled={recovering || !recoveryShareA.trim() || !recoveryShareB.trim()}
                    className="flex h-9 w-full items-center justify-center gap-2 rounded-[3px] bg-[var(--text-primary)] text-[12px] font-semibold text-[var(--bg-base)] transition-all hover:opacity-90 active:scale-[0.99] disabled:opacity-50 mt-1 cursor-pointer disabled:cursor-not-allowed"
                  >
                    {recovering ? (
                      <>
                        <Loader2 size={13} className="animate-spin" />
                        <span>Reconstructing...</span>
                      </>
                    ) : (
                      <span>{t('login.reconstruct_and_unlock')}</span>
                    )}
                  </button>
                </form>
              ) : (
                <div className="mt-3 flex w-full flex-col gap-2.5">
                  <div className="flex flex-col gap-1 rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] p-2.5 text-left">
                    <span className="text-[10px] font-mono font-semibold uppercase tracking-wider text-[var(--text-secondary)]">
                      {t('login.recovered_password_title')}
                    </span>
                    <span className="font-mono text-[12px] font-semibold text-[var(--text-primary)] break-all select-all">
                      {recoveredPassword}
                    </span>
                  </div>

                  <div className="flex gap-2">
                    <button
                      type="button"
                      onClick={() => {
                        if (isTauri()) {
                          getBackend().then(b => b.copyToClipboard(recoveredPassword, true, 30)).catch(() => {});
                        } else {
                          navigator.clipboard.writeText(recoveredPassword).catch(() => {});
                        }
                        setCopiedRecovered(true);
                        setTimeout(() => setCopiedRecovered(false), 2000);
                      }}
                      className="flex h-8.5 flex-1 items-center justify-center gap-1.5 rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] text-[11px] font-medium text-[var(--text-primary)] hover:bg-[var(--bg-hover)] cursor-pointer"
                    >
                      <Copy size={12} />
                      <span>{copiedRecovered ? (t('common.copied') || 'Copied') : (t('common.copy') || 'Copy')}</span>
                    </button>
                    <button
                      type="button"
                      onClick={handleUnlockWithRecovered}
                      disabled={loading}
                      className="flex h-8.5 flex-1 items-center justify-center gap-1.5 rounded-[3px] bg-[var(--text-primary)] text-[11px] font-semibold text-[var(--bg-base)] hover:opacity-90 active:scale-[0.99] disabled:opacity-50 cursor-pointer disabled:cursor-not-allowed"
                    >
                      {loading ? <Loader2 size={12} className="animate-spin" /> : null}
                      <span>{t('login.unlock_btn')}</span>
                    </button>
                  </div>
                </div>
              )}
            </div>

            {/* Back to master password */}
            <button
              type="button"
              onClick={() => {
                setRecoveryError('');
                setRecoveredPassword('');
                setActiveView('master_password');
              }}
              className="flex h-8.5 w-full items-center justify-center gap-1.5 rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] text-[12px] font-medium text-[var(--text-secondary)] transition-colors hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)] cursor-pointer"
            >
              <span>{t('login.back_to_password')}</span>
            </button>
          </div>
        ) : (
          /* Master Password Form */
          <form onSubmit={handleSubmit} className="mt-6 flex flex-col gap-3">
            <motion.div
              animate={shake ? { x: [0, -4, 4, -4, 4, 0] } : {}}
              transition={{ duration: 0.3 }}
            >
              {hardware2FaRequired && (
                <div className="mb-2 flex items-center justify-between">
                  <span className="text-[11px] font-medium text-[var(--text-primary)]">
                    {t('login.master_password_fallback') || 'Master Password Fallback Active'}
                  </span>
                  <button
                    type="button"
                    onClick={() => {
                      setError('');
                      setActiveView('hardware_2fa');
                    }}
                    className="text-[11px] text-[var(--text-secondary)] hover:text-[var(--text-primary)] transition-colors cursor-pointer"
                  >
                    {t('login.use_hardware_key') || 'Use Hardware Key'}
                  </button>
                </div>
              )}
              <div className="relative">
                <SecureSecretInput
                  ref={secretInputRef}
                  value={password}
                  onChange={setPassword}
                  show={showPassword}
                  disabled={loading || isLockedOut}
                  placeholder={t('login.master_password')}
                  autoFocus
                  onKeyDown={() => setError('')}
                  className="h-11 w-full text-[14px] pr-10"
                />
                <ActionTooltip content={showPassword ? t('login.hide_password') : t('login.show_password')}>
                  <button
                    type="button"
                    disabled={loading || isLockedOut}
                    onClick={() => setShowPassword(!showPassword)}
                    className="absolute right-2.5 top-1/2 -translate-y-1/2 text-[var(--text-tertiary)] transition-colors hover:text-[var(--text-primary)] disabled:opacity-40"
                  >
                    {showPassword ? <EyeOff size={16} /> : <Eye size={16} />}
                  </button>
                </ActionTooltip>
              </div>

              {/* Key File Toggle & Input */}
              <div className="mt-2.5 flex flex-col gap-2">
                <label className={`flex items-center gap-2 select-none text-[12px] font-medium text-[var(--text-secondary)] transition-colors ${loading || isLockedOut ? 'opacity-50 cursor-not-allowed' : 'cursor-pointer hover:text-[var(--text-primary)]'}`}>
                  <input
                    type="checkbox"
                    checked={useKeyFile}
                    disabled={loading || isLockedOut}
                    onChange={(e) => {
                      setUseKeyFile(e.target.checked);
                      setError('');
                    }}
                    className="rounded border-[var(--border)] text-[var(--text-primary)] accent-[var(--accent)]"
                  />
                  <KeyRound size={13} className="text-[var(--text-tertiary)]" />
                  <span>{t('login.use_key_file')}</span>
                </label>

                {useKeyFile && (
                  <div className="flex gap-1.5">
                    <input
                      type="text"
                      value={keyFilePath}
                      disabled={loading || isLockedOut}
                      onChange={(e) => {
                        setKeyFilePath(e.target.value);
                        setError('');
                      }}
                      placeholder={t('login.key_file_path')}
                      className="h-9 flex-1 rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] px-2.5 font-mono text-[12px] text-[var(--text-primary)] outline-none placeholder:font-sans placeholder:text-[var(--text-tertiary)] focus:border-[var(--border-focus)] disabled:opacity-50 disabled:cursor-not-allowed"
                    />
                    {isTauri() && (
                      <ActionTooltip content={t('login.browse_key_file')}>
                        <button
                          type="button"
                          disabled={loading || isLockedOut}
                          onClick={handleBrowseKeyFile}
                          className="flex h-9 items-center gap-1.5 rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] px-2.5 text-[12px] font-medium text-[var(--text-secondary)] transition-colors hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)] shrink-0 disabled:opacity-50 disabled:cursor-not-allowed"
                        >
                          <FolderOpen size={13} />
                          {t('common.browse')}
                        </button>
                      </ActionTooltip>
                    )}
                  </div>
                )}
              </div>

              {error && (
                <div className="mt-2 flex items-center gap-1.5">
                  <AlertTriangle size={12} className="shrink-0 text-[var(--destructive)]" />
                  <p className="text-[12px] text-[var(--destructive)] select-text">{error}</p>
                </div>
              )}
            </motion.div>

            <button
              type="submit"
              disabled={loading || isLockedOut}
              className="flex h-10 w-full items-center justify-center gap-2 rounded-[3px] bg-[var(--text-primary)] text-[13px] font-semibold text-[var(--bg-base)] transition-all hover:opacity-90 active:scale-[0.99] disabled:opacity-50"
            >
              {loading ? (
                <>
                  <Loader2 size={14} className="animate-spin" />
                  {t('login.unlocking')}
                </>
              ) : isLockedOut ? (
                t('login.locked_status', { remaining: lockoutRemaining })
              ) : (
                t('login.unlock_btn')
              )}
            </button>

            {/* Emergency Recovery Option */}
            <button
              type="button"
              onClick={() => {
                setError('');
                setActiveView('emergency_recovery');
              }}
              disabled={loading}
              className="mt-1 text-center text-[11px] text-[var(--text-tertiary)] hover:text-[var(--text-secondary)] transition-colors underline-offset-2 hover:underline cursor-pointer disabled:opacity-50"
            >
              {t('login.emergency_recovery_btn')}
            </button>

            {/* Switch to Biometric Primary if available */}
            {biometricAvailable && (
              <div className="mt-1">
                <button
                  type="button"
                  onClick={() => {
                    setError('');
                    setActiveView('biometric');
                    handleUnlockBiometric();
                  }}
                  disabled={loading || isLockedOut}
                  className="w-full flex h-8 items-center justify-center gap-1.5 rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] text-[12px] font-medium text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:bg-[var(--bg-hover)] transition-colors cursor-pointer disabled:opacity-50"
                >
                  <Fingerprint size={13} className="text-[var(--accent)]" />
                  <span>{t('login.use_biometric', { bioType: biometricType.split('(')[0].trim() }) || `Unlock with ${biometricType.split('(')[0].trim()}`}</span>
                </button>
              </div>
            )}

            {/* Switch to YubiKey Primary if available */}
            {hardware2FaRequired && (
              <div className="mt-1">
                <button
                  type="button"
                  onClick={() => {
                    setError('');
                    setActiveView('hardware_2fa');
                  }}
                  disabled={loading || isLockedOut}
                  className="w-full flex h-8 items-center justify-center gap-1.5 rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] text-[12px] font-medium text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:bg-[var(--bg-hover)] transition-colors"
                >
                  <ShieldCheck size={13} />
                  <span>{t('login.switch_to_hardware_key') || 'Switch to Hardware Key Primary'}</span>
                </button>
              </div>
            )}
          </form>
        )}

        {/* Attempts warning */}
        {attempts >= 3 && (
          <div className="mt-3 rounded-[3px] bg-[var(--destructive)]/10 px-3 py-2 text-center text-[11px] text-[var(--destructive)]">
            {t('login.attempts_left', { remaining: MAX_ATTEMPTS - attempts })}
          </div>
        )}

        {/* Back link */}
        <button
          onClick={() => {
            setCurrentVault(null);
            navigate('/', { state: { manualSelect: true } });
          }}
          className="mx-auto mt-4 block text-[12px] text-[var(--text-secondary)] transition-colors hover:text-[var(--text-primary)]"
        >
          {t('login.back_to_vaults')}
        </button>
      </div>
    </motion.div>
  );
}



