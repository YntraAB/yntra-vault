import { useState, useCallback, useRef, useEffect } from 'react';
import { motion } from 'framer-motion';
import { Eye, EyeOff, Loader2, AlertTriangle } from 'lucide-react';
import { useNavigate } from 'react-router-dom';
import { useAppState } from '@/contexts/AppStateContext';
import { isTauri, getBackend } from '@/lib/backend';

const MAX_ATTEMPTS = 5;
const LOCKOUT_DELAYS = [0, 0, 0, 5000, 15000, 30000]; // ms delay per attempt

export default function Login() {
  const navigate = useNavigate();
  const { currentVault, setIsLocked } = useAppState();
  const [password, setPassword] = useState('');
  const [showPassword, setShowPassword] = useState(false);
  const [error, setError] = useState('');
  const [shake, setShake] = useState(false);
  const [loading, setLoading] = useState(false);
  const [attempts, setAttempts] = useState(0);
  const [lockedUntil, setLockedUntil] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);

  const isLockedOut = Date.now() < lockedUntil;
  const lockoutRemaining = Math.ceil((lockedUntil - Date.now()) / 1000);

  // Redirect if not in Tauri desktop mode or no vault is selected
  useEffect(() => {
    if (!isTauri() || !currentVault) {
      navigate('/');
    }
  }, [currentVault, navigate]);

  const triggerShake = () => {
    setShake(true);
    setTimeout(() => setShake(false), 300);
  };

  const handleSubmit = useCallback(
    async (e: React.FormEvent) => {
      e.preventDefault();
      setError('');

      if (isLockedOut) {
        setError(`Too many attempts. Try again in ${lockoutRemaining}s`);
        return;
      }

      if (!password.trim()) {
        setError('Enter your master password');
        triggerShake();
        return;
      }

      setLoading(true);
      try {
        if (isTauri() && currentVault) {
          const backend = await getBackend();
          await backend.openVault(currentVault.path, password);
        }

        // Success
        setIsLocked(false);
        setPassword(''); // Security: clear from state
        setAttempts(0);
        navigate('/app');
      } catch (err: any) {
        const newAttempts = attempts + 1;
        setAttempts(newAttempts);

        // Rate limiting
        const delay = LOCKOUT_DELAYS[Math.min(newAttempts, LOCKOUT_DELAYS.length - 1)];
        if (delay > 0) {
          setLockedUntil(Date.now() + delay);
          setError(`Incorrect password. Locked for ${delay / 1000}s`);

          // Auto-unlock countdown
          setTimeout(() => {
            setLockedUntil(0);
            setError('');
            inputRef.current?.focus();
          }, delay);
        } else {
          setError('Incorrect password');
        }

        triggerShake();
        setPassword('');
      } finally {
        setLoading(false);
      }
    },
    [password, setIsLocked, navigate, currentVault, attempts, isLockedOut, lockoutRemaining]
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
        <div className="text-center">
          <h1 className="text-[18px] font-semibold tracking-tight text-[var(--text-primary)]">
            {currentVault?.name || 'Vault'}
          </h1>
          <p className="mt-1 text-[13px] text-[var(--text-secondary)]">
            Enter master password to unlock
          </p>
        </div>

        {/* Form */}
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
                placeholder="Master password"
                autoFocus
                disabled={loading || isLockedOut}
                className={`h-11 w-full rounded-[3px] border bg-[var(--bg-elevated)] px-3 pr-10 font-mono text-[14px] tracking-wider text-[var(--text-primary)] outline-none transition-colors placeholder:text-[var(--text-tertiary)] placeholder:font-sans placeholder:tracking-normal disabled:opacity-50 ${
                  error
                    ? 'border-[var(--destructive)]'
                    : 'border-[var(--border)] focus:border-[var(--border-focus)]'
                }`}
              />
              <button
                type="button"
                onClick={() => setShowPassword(!showPassword)}
                className="absolute right-2.5 top-1/2 -translate-y-1/2 text-[var(--text-tertiary)] transition-colors hover:text-[var(--text-primary)]"
              >
                {showPassword ? <EyeOff size={16} /> : <Eye size={16} />}
              </button>
            </div>
            {error && (
              <div className="mt-1.5 flex items-center gap-1.5">
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
                Unlocking...
              </>
            ) : isLockedOut ? (
              `Locked (${lockoutRemaining}s)`
            ) : (
              'Unlock Vault'
            )}
          </button>
        </form>

        {/* Attempts warning */}
        {attempts >= 3 && (
          <div className="mt-3 rounded-md bg-[var(--destructive)]/10 px-3 py-2 text-center text-[11px] text-[var(--destructive)]">
            {MAX_ATTEMPTS - attempts} attempts remaining before extended lockout
          </div>
        )}

        {/* Back link */}
        <button
          onClick={() => navigate('/')}
          className="mx-auto mt-4 block text-[12px] text-[var(--text-secondary)] transition-colors hover:text-[var(--text-primary)]"
        >
          &larr; Back to vaults
        </button>
      </div>
    </motion.div>
  );
}



