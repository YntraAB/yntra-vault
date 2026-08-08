import { useState, useCallback, useRef, useEffect } from 'react';
import { motion } from 'framer-motion';
import { Eye, EyeOff, Loader2, AlertTriangle, KeyRound, FolderOpen, Fingerprint } from 'lucide-react';
import { useNavigate } from 'react-router-dom';
import { useAppState } from '@/contexts/AppStateContext';
import { useTranslation } from '@/contexts/LanguageContext';
import { isTauri, getBackend } from '@/lib/backend';
import { ActionTooltip } from '@/components/ui/tooltip';

const MAX_ATTEMPTS = 5;
const LOCKOUT_DELAYS = [0, 0, 0, 5000, 15000, 30000]; // ms delay per attempt

export default function Login() {
  const navigate = useNavigate();
  const { t } = useTranslation();
  const { currentVault, setIsLocked, setCurrentVault } = useAppState();
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
  const [biometricType, setBiometricType] = useState('Biometrics');
  const [hardware2FaRequired, setHardware2FaRequired] = useState(false);
  const [activeView, setActiveView] = useState<'master_password' | 'biometric' | 'hardware_2fa'>('master_password');
  const inputRef = useRef<HTMLInputElement>(null);

  const isLockedOut = Date.now() < lockedUntil;
  const lockoutRemaining = Math.ceil((lockedUntil - Date.now()) / 1000);

  const triggerShake = useCallback(() => {
    setShake(true);
    setTimeout(() => setShake(false), 300);
  }, []);

  const handleBiometricUnlock = useCallback(async () => {
    if (!currentVault?.path) return;
    setLoading(true);
    setError('');
    try {
      const backend = await getBackend();
      const info = await backend.unlockVaultBiometric(currentVault.path);
      const recent = JSON.parse(localStorage.getItem('yntra-vault-recent-vaults') || '[]');
      const updated = recent.filter((v: any) => v.id !== info.id && v.path !== info.path);
      const newVault = { id: info.id, name: info.name, path: info.path };
      localStorage.setItem('yntra-vault-recent-vaults', JSON.stringify([newVault, ...updated.slice(0, 9)]));
      setCurrentVault(newVault);
      setIsLocked(false);
      navigate('/app');
    } catch (err: any) {
      const msg = err.toString() || 'Biometric authentication failed';
      if (!msg.includes('canceled')) {
        setError(msg);
        triggerShake();
      }
    } finally {
      setLoading(false);
    }
  }, [currentVault, navigate, setCurrentVault, setIsLocked, triggerShake]);

  // Redirect if not in Tauri desktop mode or no vault is selected & check biometrics / Hardware 2FA
  useEffect(() => {
    if (!isTauri() || !currentVault) {
      navigate('/');
    } else if (currentVault?.path) {
      getBackend().then(async (backend) => {
        try {
          const hwEnabled = await backend.isHardware2FaEnabled(currentVault.path);
          setHardware2FaRequired(hwEnabled);

          const enabled = await backend.isBiometricEnabled(currentVault.path);
          if (enabled) {
            const info = await backend.checkBiometricAvailable();
            setBiometricAvailable(info.available);
            if (info.biometric_type) setBiometricType(info.biometric_type);
          }

          const savedPrimary = localStorage.getItem(`yntra-vault-primary-unlock-${currentVault.path}`);
          if (savedPrimary === 'biometric' && enabled) {
            setActiveView('biometric');
          } else if (savedPrimary === 'hardware_2fa' && hwEnabled) {
            setActiveView('hardware_2fa');
          }
        } catch (e) {
          console.error('Security check error:', e);
        }
      });
    }
  }, [currentVault, navigate]);

  const handleBrowseKeyFile = async () => {
    if (!isTauri()) return;
    try {
      const { open } = await import('@tauri-apps/plugin-dialog');
      const selected = await open({
        title: 'Select Key File',
        multiple: false,
        filters: [{ name: 'Key File (*.key, *.*)', extensions: ['key', '*'] }],
      });
      if (selected) {
        setKeyFilePath(typeof selected === 'string' ? selected : String(selected));
      }
    } catch (e) {
      console.error('Key file selection failed:', e);
    }
  };

  const handleSubmit = useCallback(
    async (e: React.FormEvent) => {
      e.preventDefault();
      setError('');

      if (isLockedOut) {
        setError(`Too many attempts. Try again in ${lockoutRemaining}s`);
        return;
      }

      // If biometrics is available and no password typed, trigger 1-click biometric unlock
      if (biometricAvailable && !password.trim() && !useKeyFile) {
        handleBiometricUnlock();
        return;
      }

      if (!password.trim()) {
        setError('Enter your master password');
        triggerShake();
        return;
      }

      if (useKeyFile && !keyFilePath.trim()) {
        setError('Please select or specify a Key File');
        triggerShake();
        return;
      }

      setLoading(true);
      try {
        let info;
        if (isTauri() && currentVault) {
          const backend = await getBackend();
          const kf = useKeyFile && keyFilePath.trim() ? keyFilePath.trim() : undefined;
          if (hardware2FaRequired) {
            const sampleChallenge = Array.from(crypto.getRandomValues(new Uint8Array(32)));
            const hwResp = await backend.performHardware2FaChallenge('YubiKeyChallengeResponse', sampleChallenge);
            info = await backend.openVaultWithHardware2Fa(currentVault.path, password, kf, hwResp);
          } else {
            info = await backend.openVault(currentVault.path, password, kf);
          }
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
          setError('Hardware 2FA required. YubiKey challenge initialized — press Submit again.');
          triggerShake();
        } else {
          const newAttempts = attempts + 1;
          setAttempts(newAttempts);

          // Rate limiting
          const delay = LOCKOUT_DELAYS[Math.min(newAttempts, LOCKOUT_DELAYS.length - 1)];
          if (delay > 0) {
            setLockedUntil(Date.now() + delay);
            setError(`Incorrect password or key file. Locked for ${delay / 1000}s`);

            // Auto-unlock countdown
            setTimeout(() => {
              setLockedUntil(0);
              setError('');
              inputRef.current?.focus();
            }, delay);
          } else {
            setError('Incorrect password or invalid key file');
          }

          triggerShake();
          setPassword('');
        }
      } finally {
        setLoading(false);
      }
    },
    [password, useKeyFile, keyFilePath, hardware2FaRequired, setIsLocked, setCurrentVault, navigate, currentVault, attempts, isLockedOut, lockoutRemaining]
  );

  return (
    <motion.div
      initial={{ opacity: 0, y: 8 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ duration: 0.2, ease: 'easeOut' }}
      className="flex h-screen w-screen items-center justify-center bg-[var(--bg-base)]"
    >
      <div className="w-[360px] px-6">
        {/* Header */}
        <div className="flex flex-col items-center text-center">
          <img
            src="/white-logo.png"
            alt="Yntra Vault Logo"
            className="mb-3 h-24 w-24 rounded-xl object-cover"
          />
          <h1 className="text-[18px] font-semibold tracking-tight text-[var(--text-primary)]">
            {currentVault?.name || 'Vault'}
          </h1>
          {hardware2FaRequired && (
            <div className="mt-2 inline-flex items-center gap-1.5 rounded-full border border-cyan-500/30 bg-cyan-500/10 px-2.5 py-1 text-[11px] font-medium text-cyan-400">
              <KeyRound size={12} />
              <span>Hardware 2FA / YubiKey Required</span>
            </div>
          )}
          <p className="mt-1 text-[13px] text-[var(--text-secondary)]">
            {t('login.enter_master')}
          </p>
        </div>

        {/* Views depending on primary login choice */}
        {activeView === 'biometric' && biometricAvailable ? (
          <div className="mt-6 flex flex-col gap-3">
            <div className="flex flex-col items-center rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] p-5 text-center shadow-sm">
              <div className="mb-3 flex h-12 w-12 items-center justify-center rounded-full border border-[var(--border)] bg-[var(--bg-base)] text-[var(--text-primary)]">
                <Fingerprint size={24} />
              </div>
              <h2 className="text-[15px] font-semibold tracking-tight text-[var(--text-primary)]">
                {biometricType.replace(/\s*\(.*\)/, '') || 'Windows Hello'}
              </h2>
              <p className="mt-1 text-[12px] text-[var(--text-secondary)]">
                Verify your identity using hardware biometrics
              </p>

              {error && (
                <div className="mt-3 flex items-center gap-1.5 rounded-[3px] bg-[var(--destructive)]/10 px-3 py-1.5 text-[12px] text-[var(--destructive)]">
                  <AlertTriangle size={13} className="shrink-0" />
                  <span>{error}</span>
                </div>
              )}

              <ActionTooltip content={t('login.biometric_tooltip')}>
                <button
                  type="button"
                  onClick={handleBiometricUnlock}
                  disabled={loading || isLockedOut}
                  className="mt-4 flex h-10 w-full items-center justify-center gap-2 rounded-[3px] bg-[var(--text-primary)] text-[13px] font-semibold text-[var(--bg-base)] transition-all hover:opacity-90 active:scale-[0.99] disabled:opacity-50"
                >
                  {loading ? (
                    <>
                      <Loader2 size={15} className="animate-spin" />
                      <span>{t('login.unlocking')}</span>
                    </>
                  ) : (
                    <>
                      <Fingerprint size={15} />
                      <span>Unlock with {biometricType.replace(/\s*\(.*\)/, '') || 'Windows Hello'}</span>
                    </>
                  )}
                </button>
              </ActionTooltip>
            </div>

            {/* Alternative Login Options */}
            <div className="flex items-center justify-center gap-2">
              <button
                type="button"
                onClick={() => {
                  setError('');
                  setActiveView('master_password');
                }}
                className="flex h-8 flex-1 items-center justify-center gap-1.5 rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] px-3 text-[12px] font-medium text-[var(--text-secondary)] transition-colors hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"
              >
                <KeyRound size={13} />
                <span>Master Password</span>
              </button>
              {hardware2FaRequired && (
                <button
                  type="button"
                  onClick={() => {
                    setError('');
                    setActiveView('hardware_2fa');
                  }}
                  className="flex h-8 flex-1 items-center justify-center gap-1.5 rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] px-3 text-[12px] font-medium text-[var(--text-secondary)] transition-colors hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"
                >
                  <KeyRound size={13} />
                  <span>YubiKey 2FA</span>
                </button>
              )}
            </div>
          </div>
        ) : (
          /* Form */
          <form onSubmit={handleSubmit} className="mt-6 flex flex-col gap-3">
            <motion.div
              animate={shake ? { x: [0, -4, 4, -4, 4, 0] } : {}}
              transition={{ duration: 0.3 }}
            >
              <div className="relative">
                <input
                  ref={inputRef}
                  type={showPassword ? 'text' : 'password'}
                  value={password}
                  onChange={(e) => {
                    setPassword(e.target.value);
                    setError('');
                  }}
                  placeholder={t('login.master_password')}
                  autoFocus
                  disabled={loading || isLockedOut}
                  className={`h-11 w-full rounded-[3px] border bg-[var(--bg-elevated)] px-3 pr-10 font-mono text-[14px] tracking-wider text-[var(--text-primary)] outline-none transition-colors placeholder:text-[var(--text-tertiary)] placeholder:font-sans placeholder:tracking-normal disabled:opacity-50 ${
                    error
                      ? 'border-[var(--destructive)]'
                      : 'border-[var(--border)] focus:border-[var(--border-focus)]'
                  }`}
                />
                <ActionTooltip content={showPassword ? t('login.hide_password') : t('login.show_password')}>
                  <button
                    type="button"
                    onClick={() => setShowPassword(!showPassword)}
                    className="absolute right-2.5 top-1/2 -translate-y-1/2 text-[var(--text-tertiary)] transition-colors hover:text-[var(--text-primary)]"
                  >
                    {showPassword ? <EyeOff size={16} /> : <Eye size={16} />}
                  </button>
                </ActionTooltip>
              </div>

              {/* Key File Toggle & Input */}
              <div className="mt-2.5 flex flex-col gap-2">
                <label className="flex items-center gap-2 cursor-pointer select-none text-[12px] font-medium text-[var(--text-secondary)] hover:text-[var(--text-primary)] transition-colors">
                  <input
                    type="checkbox"
                    checked={useKeyFile}
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
                      onChange={(e) => {
                        setKeyFilePath(e.target.value);
                        setError('');
                      }}
                      placeholder={t('login.key_file_path')}
                      className="h-9 flex-1 rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] px-2.5 font-mono text-[12px] text-[var(--text-primary)] outline-none placeholder:font-sans placeholder:text-[var(--text-tertiary)] focus:border-[var(--border-focus)]"
                    />
                    {isTauri() && (
                      <ActionTooltip content={t('login.browse_key_file')}>
                        <button
                          type="button"
                          onClick={handleBrowseKeyFile}
                          className="flex h-9 items-center gap-1.5 rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] px-2.5 text-[12px] font-medium text-[var(--text-secondary)] transition-colors hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)] shrink-0"
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
                  <p className="text-[12px] text-[var(--destructive)]">{error}</p>
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

            {/* Quick Switch Alternative Options */}
            <div className="mt-1 flex gap-2">
              {biometricAvailable && activeView !== 'biometric' && (
                <button
                  type="button"
                  onClick={() => {
                    setError('');
                    setActiveView('biometric');
                  }}
                  disabled={loading || isLockedOut}
                  className="flex-1 flex h-8 items-center justify-center gap-1.5 rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] text-[12px] font-medium text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:bg-[var(--bg-hover)] transition-colors"
                >
                  <Fingerprint size={13} />
                  <span>{biometricType.replace(/\s*\(.*\)/, '') || 'Windows Hello'}</span>
                </button>
              )}
              {hardware2FaRequired && activeView !== 'hardware_2fa' && (
                <button
                  type="button"
                  onClick={() => {
                    setError('');
                    setActiveView('hardware_2fa');
                  }}
                  disabled={loading || isLockedOut}
                  className="flex-1 flex h-8 items-center justify-center gap-1.5 rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] text-[12px] font-medium text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:bg-[var(--bg-hover)] transition-colors"
                >
                  <KeyRound size={13} />
                  <span>YubiKey 2FA</span>
                </button>
              )}
            </div>
          </form>
        )}

        {/* Attempts warning */}
        {attempts >= 3 && (
          <div className="mt-3 rounded-md bg-[var(--destructive)]/10 px-3 py-2 text-center text-[11px] text-[var(--destructive)]">
            {t('login.attempts_left', { remaining: MAX_ATTEMPTS - attempts })}
          </div>
        )}

        {/* Back link */}
        <button
          onClick={() => navigate('/')}
          className="mx-auto mt-4 block text-[12px] text-[var(--text-secondary)] transition-colors hover:text-[var(--text-primary)]"
        >
          {t('login.back_to_vaults')}
        </button>
      </div>
    </motion.div>
  );
}



