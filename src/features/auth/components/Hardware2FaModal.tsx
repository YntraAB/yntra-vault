import { useState, useEffect } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { KeyRound, ShieldCheck, Cpu, RefreshCw, X, CheckCircle2, AlertCircle } from 'lucide-react';
import { getBackend, isTauri, openFileDialog } from '@/lib/backend';
import type { Hardware2FaProtocol, HardwareKeyInfo } from '@/lib/backend';
import { ActionTooltip } from '@/components/ui/tooltip';
import { useTranslation } from '@/contexts/LanguageContext';
import { useAuth } from '@/features/auth';

export interface Hardware2FaModalProps {
  open: boolean;
  onClose: () => void;
  onSuccess?: () => void;
  mode?: 'enroll' | 'test';
}

export function Hardware2FaModal({ open, onClose, onSuccess, mode = 'enroll' }: Hardware2FaModalProps) {
  const { t } = useTranslation();
  const { currentVault } = useAuth();
  const [step, setStep] = useState<'select' | 'prompt' | 'success'>('select');
  const [protocol, setProtocol] = useState<Hardware2FaProtocol>('YubiKeyChallengeResponse');
  const [keyName, setKeyName] = useState('My YubiKey 5');
  const [masterPassword, setMasterPassword] = useState('');
  const [useKeyFile, setUseKeyFile] = useState(false);
  const [keyFilePath, setKeyFilePath] = useState('');
  const [keys, setKeys] = useState<HardwareKeyInfo[]>([]);
  const [loading, setLoading] = useState(false);
  const [refreshing, setRefreshing] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const refreshKeys = async () => {
    if (!isTauri()) return;
    setRefreshing(true);
    setError(null);
    try {
      const backend = await getBackend();
      const list = await backend.listHardwareKeys();
      setKeys(list);
      if (list.length > 0) {
        setKeyName(list[0].name);
        setProtocol(list[0].protocol);
      }
    } catch (e: any) {
      setError(e.toString());
    } finally {
      setRefreshing(false);
    }
  };

  useEffect(() => {
    if (open && isTauri()) {
      refreshKeys();
      setStep('select');
      setError(null);
      setMasterPassword('');
      if (currentVault?.path) {
        try {
          const savedKeyFiles = JSON.parse(localStorage.getItem('yntra-vault-keyfiles') || '{}');
          const kf = savedKeyFiles[currentVault.path] || (currentVault as any)?.keyFilePath;
          if (kf) {
            setUseKeyFile(true);
            setKeyFilePath(kf);
          } else {
            setUseKeyFile(false);
            setKeyFilePath('');
          }
        } catch {
          setUseKeyFile(false);
          setKeyFilePath('');
        }
      }
    }
  }, [open, currentVault]);

  const handleBrowseKeyFile = async () => {
    if (!isTauri()) return;
    try {
      const selected = await openFileDialog({
        multiple: false,
        directory: false,
        title: 'Select Key File',
      });
      if (typeof selected === 'string' && selected.trim()) {
        setKeyFilePath(selected.trim());
        setUseKeyFile(true);
      }
    } catch (e) {
      console.error('Failed to select key file:', e);
    }
  };

  const handleStartChallenge = async () => {
    if (mode === 'enroll' && !masterPassword.trim()) {
      setError('Please enter your master password to authorize enrollment');
      return;
    }

    setLoading(true);
    setError(null);
    setStep('prompt');

    try {
      const backend = await getBackend();
      const challengeSalt = Array.from(crypto.getRandomValues(new Uint8Array(32)));
      const responseBytes = await backend.performHardware2FaChallenge(protocol, challengeSalt);

      if (mode === 'enroll') {
        const kf = useKeyFile && keyFilePath.trim() ? keyFilePath.trim() : undefined;
        await backend.enableHardware2Fa(
          masterPassword,
          kf,
          protocol,
          keyName,
          challengeSalt,
          undefined,
          responseBytes,
        );
        setMasterPassword('');
      }

      setStep('success');
      setTimeout(() => {
        onSuccess?.();
        onClose();
      }, 1200);
    } catch (err: any) {
      setError(err.toString() || 'Hardware key authentication failed');
      setStep('select');
    } finally {
      setLoading(false);
    }
  };

  if (!open) return null;

  return (
    <AnimatePresence>
      <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm select-none">
        <motion.div
          initial={{ opacity: 0, scale: 0.95 }}
          animate={{ opacity: 1, scale: 1 }}
          exit={{ opacity: 0, scale: 0.95 }}
          className="w-full max-w-[440px] mx-3 rounded-xl border border-[var(--border)] bg-[var(--bg-elevated)] p-6 shadow-2xl"
        >
          {/* Header */}
          <div className="flex items-center justify-between border-b border-[var(--border)] pb-4">
            <div className="flex items-center gap-3">
              <div className="flex h-10 w-10 items-center justify-center rounded-lg border border-[var(--border)] bg-[var(--accent-bg)] text-[var(--text-primary)]">
                <KeyRound size={20} />
              </div>
              <div>
                <h2 className="text-[16px] font-semibold text-[var(--text-primary)]">
                  {mode === 'enroll' ? t('hw.enroll_title') : t('hw.test_title')}
                </h2>
                <p className="text-[12px] text-[var(--text-secondary)]">
                  YubiKey (CTAP1 HMAC-SHA1) & FIDO2 (CTAP2 HMAC-Secret)
                </p>
              </div>
            </div>
            <ActionTooltip content={t('common.close')}>
              <button
                onClick={onClose}
                className="rounded-lg p-1.5 text-[var(--text-tertiary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)] transition-colors"
              >
                <X size={16} />
              </button>
            </ActionTooltip>
          </div>

          {/* Content */}
          <div className="py-6">
            {step === 'select' && (
              <div className="flex flex-col gap-4">
                <div>
                  <label className="text-[12px] font-medium text-[var(--text-secondary)]">{t('hw.protocol_label')}</label>
                  <div className="mt-1.5 grid grid-cols-2 gap-2">
                    <button
                      type="button"
                      onClick={() => setProtocol('YubiKeyChallengeResponse')}
                      className={`flex flex-col items-center gap-2 rounded-lg border p-3 text-center transition-all ${
                        protocol === 'YubiKeyChallengeResponse'
                          ? 'border-[var(--accent)] bg-[var(--accent-bg)] text-[var(--text-primary)] font-medium'
                          : 'border-[var(--border)] bg-[var(--bg-base)] text-[var(--text-secondary)] hover:border-[var(--border-focus)]'
                      }`}
                    >
                      <Cpu size={20} />
                      <span className="text-[12px]">YubiKey Challenge-Response</span>
                    </button>
                    <button
                      type="button"
                      onClick={() => setProtocol('Fido2Ctap2HmacSecret')}
                      className={`flex flex-col items-center gap-2 rounded-lg border p-3 text-center transition-all ${
                        protocol === 'Fido2Ctap2HmacSecret'
                          ? 'border-[var(--accent)] bg-[var(--accent-bg)] text-[var(--text-primary)] font-medium'
                          : 'border-[var(--border)] bg-[var(--bg-base)] text-[var(--text-secondary)] hover:border-[var(--border-focus)]'
                      }`}
                    >
                      <ShieldCheck size={20} />
                      <span className="text-[12px]">FIDO2 / CTAP2 (HMAC-Secret)</span>
                    </button>
                  </div>
                </div>

                {mode === 'enroll' && (
                  <>
                    <div className="rounded-lg border border-amber-500/20 bg-amber-500/10 p-3 text-[12px] text-amber-300">
                      <p className="font-medium">Strict Two-Factor Authentication</p>
                      <p className="mt-1 text-[11px] text-amber-200/80">
                        Enrolling a security key binds your master password and physical key together. Single-factor biometric unlock will be disabled.
                      </p>
                    </div>

                    <div>
                      <label className="text-[12px] font-medium text-[var(--text-secondary)]">Master Password (Required)</label>
                      <input
                        type="password"
                        value={masterPassword}
                        onChange={(e) => setMasterPassword(e.target.value)}
                        placeholder="Enter master password to authorize"
                        className="mt-1.5 h-10 w-full rounded-md border border-[var(--border)] bg-[var(--bg-base)] px-3 text-[13px] text-[var(--text-primary)] outline-none focus:border-[var(--border-focus)]"
                      />
                    </div>

                    <div>
                      <div className="flex items-center justify-between">
                        <label className="text-[12px] font-medium text-[var(--text-secondary)]">Key File (Optional)</label>
                        {keyFilePath && (
                          <button
                            type="button"
                            onClick={() => {
                              setKeyFilePath('');
                              setUseKeyFile(false);
                            }}
                            className="text-[11px] text-[var(--text-tertiary)] hover:text-red-400 cursor-pointer"
                          >
                            Remove
                          </button>
                        )}
                      </div>
                      <div className="mt-1.5 flex gap-2">
                        <input
                          type="text"
                          readOnly
                          value={keyFilePath ? keyFilePath.split(/[\\/]/).pop() || keyFilePath : ''}
                          placeholder="No key file attached"
                          className="h-10 flex-1 rounded-md border border-[var(--border)] bg-[var(--bg-base)] px-3 text-[12px] text-[var(--text-primary)] outline-none"
                        />
                        <button
                          type="button"
                          onClick={handleBrowseKeyFile}
                          className="h-10 rounded-md border border-[var(--border)] bg-[var(--bg-hover)] px-3 text-[12px] font-medium text-[var(--text-primary)] hover:bg-[var(--bg-base)] cursor-pointer"
                        >
                          Browse
                        </button>
                      </div>
                    </div>

                    <div>
                      <label className="text-[12px] font-medium text-[var(--text-secondary)]">{t('hw.key_nickname')}</label>
                      <input
                        type="text"
                        value={keyName}
                        onChange={(e) => setKeyName(e.target.value)}
                        placeholder={t('hardware_2fa.device_name_placeholder')}
                        className="mt-1.5 h-10 w-full rounded-md border border-[var(--border)] bg-[var(--bg-base)] px-3 text-[13px] text-[var(--text-primary)] outline-none focus:border-[var(--border-focus)]"
                      />
                    </div>
                  </>
                )}

                <div className="rounded-lg border border-[var(--border)] bg-[var(--bg-base)] p-3">
                  <div className="flex items-center justify-between">
                    <span className="text-[11px] font-semibold uppercase tracking-wider text-[var(--text-tertiary)]">
                      {t('hw.detected_authenticators')}
                    </span>
                    <button
                      type="button"
                      onClick={refreshKeys}
                      disabled={refreshing}
                      className="inline-flex items-center gap-1 text-[11px] text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:underline cursor-pointer disabled:opacity-50"
                    >
                      <RefreshCw size={11} className={refreshing ? 'animate-spin' : ''} />
                      <span>{refreshing ? 'Scanning...' : 'Scan Again'}</span>
                    </button>
                  </div>
                  {keys.length > 0 ? (
                    <div className="mt-2 flex flex-col gap-1.5">
                      {keys.map((k) => (
                        <div key={k.id} className="flex items-center justify-between text-[12px]">
                          <span className="font-medium text-[var(--text-primary)]">{k.name}</span>
                          <span className="inline-flex items-center gap-1 rounded bg-[var(--accent-bg)] px-2 py-0.5 font-mono text-[10px] text-[var(--text-primary)] border border-[var(--border)]">
                            {t('hw.connected')}
                          </span>
                        </div>
                      ))}
                    </div>
                  ) : (
                    <p className="mt-2 text-[12px] text-[var(--text-secondary)]">
                      No security keys detected. Connect your YubiKey or FIDO2 key to your USB port, then scan again.
                    </p>
                  )}
                </div>

                {error && (
                  <div className="flex items-center gap-2 rounded-lg border border-red-500/20 bg-red-500/10 p-3 text-[12px] text-red-400">
                    <AlertCircle size={16} className="shrink-0" />
                    <span>{error}</span>
                  </div>
                )}
              </div>
            )}

            {step === 'prompt' && (
              <div className="flex flex-col items-center justify-center py-4 text-center">
                <motion.div
                  animate={{ scale: [1, 1.15, 1], opacity: [0.8, 1, 0.8] }}
                  transition={{ duration: 1.5, repeat: Infinity }}
                  className="flex h-20 w-20 items-center justify-center rounded-full border border-[var(--border)] bg-[var(--accent-bg)] text-[var(--text-primary)] shadow-lg shadow-black/20"
                >
                  <KeyRound size={36} />
                </motion.div>
                <h3 className="mt-4 text-[15px] font-semibold text-[var(--text-primary)]">
                  {t('hw.touch_prompt_title')}
                </h3>
                <p className="mt-1 max-w-[280px] text-[12px] text-[var(--text-secondary)]">
                  {t('hw.touch_prompt_desc')}
                </p>
                <RefreshCw size={16} className="mt-4 animate-spin text-[var(--text-secondary)]" />
              </div>
            )}

            {step === 'success' && (
              <div className="flex flex-col items-center justify-center py-4 text-center">
                <div className="flex h-16 w-16 items-center justify-center rounded-full border border-emerald-500/30 bg-emerald-500/10 text-emerald-400">
                  <CheckCircle2 size={32} />
                </div>
                <h3 className="mt-3 text-[15px] font-semibold text-[var(--text-primary)]">
                  {mode === 'enroll' ? t('hw.enrolled_success') : t('hw.auth_success')}
                </h3>
                <p className="mt-1 text-[12px] text-[var(--text-secondary)]">
                  {t('hw.verified_desc')}
                </p>
              </div>
            )}
          </div>

          {/* Footer */}
          {step === 'select' && (
            <div className="flex justify-end gap-2 border-t border-[var(--border)] pt-4">
              <button
                type="button"
                onClick={onClose}
                className="h-9 rounded-md border border-[var(--border)] px-4 text-[12px] font-medium text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)] transition-colors"
              >
                {t('common.cancel')}
              </button>
              <button
                type="button"
                onClick={handleStartChallenge}
                disabled={loading || keys.length === 0 || (mode === 'enroll' && !masterPassword.trim())}
                className="flex h-9 items-center gap-2 rounded-md bg-[var(--accent)] px-4 text-[12px] font-semibold text-[var(--bg-base)] hover:bg-[var(--accent-hover)] transition-colors disabled:opacity-50 cursor-pointer disabled:cursor-not-allowed"
              >
                {mode === 'enroll' ? t('hw.start_enrollment') : t('hw.test_challenge')}
              </button>
            </div>
          )}
        </motion.div>
      </div>
    </AnimatePresence>
  );
}

export default Hardware2FaModal;
