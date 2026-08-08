import { useState, useEffect } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { KeyRound, ShieldCheck, Cpu, RefreshCw, X, CheckCircle2, AlertCircle } from 'lucide-react';
import { getBackend, isTauri } from '@/lib/backend';
import type { Hardware2FaProtocol, HardwareKeyInfo } from '@/lib/backend';
import { ActionTooltip } from '@/components/ui/tooltip';
import { useTranslation } from '@/contexts/LanguageContext';

interface Hardware2FaModalProps {
  open: boolean;
  onClose: () => void;
  onSuccess?: () => void;
  mode?: 'enroll' | 'test';
}

export default function Hardware2FaModal({ open, onClose, onSuccess, mode = 'enroll' }: Hardware2FaModalProps) {
  const { t } = useTranslation();
  const [step, setStep] = useState<'select' | 'prompt' | 'success'>('select');
  const [protocol, setProtocol] = useState<Hardware2FaProtocol>('YubiKeyChallengeResponse');
  const [keyName, setKeyName] = useState('My YubiKey 5');
  const [keys, setKeys] = useState<HardwareKeyInfo[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (open && isTauri()) {
      getBackend().then(async (backend) => {
        try {
          const list = await backend.listHardwareKeys();
          setKeys(list);
          if (list.length > 0) {
            setKeyName(list[0].name);
            setProtocol(list[0].protocol);
          }
        } catch (e: any) {
          setError(e.toString());
        }
      });
      setStep('select');
      setError(null);
    }
  }, [open]);

  const handleStartChallenge = async () => {
    setLoading(true);
    setError(null);
    setStep('prompt');

    try {
      const backend = await getBackend();
      const sampleChallenge = Array.from(crypto.getRandomValues(new Uint8Array(32)));
      const responseBytes = await backend.performHardware2FaChallenge(protocol, sampleChallenge);

      if (mode === 'enroll') {
        await backend.enableHardware2Fa(protocol, keyName, responseBytes);
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
      <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm">
        <motion.div
          initial={{ opacity: 0, scale: 0.95 }}
          animate={{ opacity: 1, scale: 1 }}
          exit={{ opacity: 0, scale: 0.95 }}
          className="w-[440px] rounded-xl border border-[var(--border)] bg-[var(--bg-elevated)] p-6 shadow-2xl"
        >
          {/* Header */}
          <div className="flex items-center justify-between border-b border-[var(--border)] pb-4">
            <div className="flex items-center gap-3">
              <div className="flex h-10 w-10 items-center justify-center rounded-lg border border-cyan-500/20 bg-cyan-500/10 text-cyan-400">
                <KeyRound size={20} />
              </div>
              <div>
                <h2 className="text-[16px] font-semibold text-[var(--text-primary)]">
                  {mode === 'enroll' ? 'Enroll Hardware Key' : 'Test Hardware 2FA'}
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
                  <label className="text-[12px] font-medium text-[var(--text-secondary)]">Hardware Protocol</label>
                  <div className="mt-1.5 grid grid-cols-2 gap-2">
                    <button
                      type="button"
                      onClick={() => setProtocol('YubiKeyChallengeResponse')}
                      className={`flex flex-col items-center gap-2 rounded-lg border p-3 text-center transition-all ${
                        protocol === 'YubiKeyChallengeResponse'
                          ? 'border-cyan-500 bg-cyan-500/10 text-cyan-400 font-medium'
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
                          ? 'border-cyan-500 bg-cyan-500/10 text-cyan-400 font-medium'
                          : 'border-[var(--border)] bg-[var(--bg-base)] text-[var(--text-secondary)] hover:border-[var(--border-focus)]'
                      }`}
                    >
                      <ShieldCheck size={20} />
                      <span className="text-[12px]">FIDO2 / CTAP2 (HMAC-Secret)</span>
                    </button>
                  </div>
                </div>

                {mode === 'enroll' && (
                  <div>
                    <label className="text-[12px] font-medium text-[var(--text-secondary)]">Security Key Nickname</label>
                    <input
                      type="text"
                      value={keyName}
                      onChange={(e) => setKeyName(e.target.value)}
                      placeholder="My YubiKey 5C"
                      className="mt-1.5 h-10 w-full rounded-md border border-[var(--border)] bg-[var(--bg-base)] px-3 text-[13px] text-[var(--text-primary)] outline-none focus:border-[var(--border-focus)]"
                    />
                  </div>
                )}

                {keys.length > 0 && (
                  <div className="rounded-lg border border-[var(--border)] bg-[var(--bg-base)] p-3">
                    <span className="text-[11px] font-semibold uppercase tracking-wider text-[var(--text-tertiary)]">
                      Detected Hardware Authenticators
                    </span>
                    <div className="mt-2 flex flex-col gap-1.5">
                      {keys.map((k) => (
                        <div key={k.id} className="flex items-center justify-between text-[12px]">
                          <span className="font-medium text-[var(--text-primary)]">{k.name}</span>
                          <span className="inline-flex items-center gap-1 rounded bg-emerald-500/10 px-2 py-0.5 font-mono text-[10px] text-emerald-400 border border-emerald-500/20">
                            Connected
                          </span>
                        </div>
                      ))}
                    </div>
                  </div>
                )}

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
                  className="flex h-20 w-20 items-center justify-center rounded-full border border-cyan-500/30 bg-cyan-500/10 text-cyan-400 shadow-lg shadow-cyan-500/20"
                >
                  <KeyRound size={36} />
                </motion.div>
                <h3 className="mt-4 text-[15px] font-semibold text-[var(--text-primary)]">
                  Touch Your YubiKey / Security Key
                </h3>
                <p className="mt-1 max-w-[280px] text-[12px] text-[var(--text-secondary)]">
                  Insert your security key into USB port and press the glowing button to perform challenge-response.
                </p>
                <RefreshCw size={16} className="mt-4 animate-spin text-cyan-400" />
              </div>
            )}

            {step === 'success' && (
              <div className="flex flex-col items-center justify-center py-4 text-center">
                <div className="flex h-16 w-16 items-center justify-center rounded-full border border-emerald-500/30 bg-emerald-500/10 text-emerald-400">
                  <CheckCircle2 size={32} />
                </div>
                <h3 className="mt-3 text-[15px] font-semibold text-[var(--text-primary)]">
                  {mode === 'enroll' ? 'Hardware Key Enrolled!' : 'Authentication Successful!'}
                </h3>
                <p className="mt-1 text-[12px] text-[var(--text-secondary)]">
                  Hardware 2FA cryptographic challenge verified cleanly.
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
                Cancel
              </button>
              <button
                type="button"
                onClick={handleStartChallenge}
                disabled={loading}
                className="flex h-9 items-center gap-2 rounded-md bg-cyan-500 px-4 text-[12px] font-semibold text-black hover:bg-cyan-400 transition-colors disabled:opacity-50"
              >
                {mode === 'enroll' ? 'Start Enrollment' : 'Test Key Challenge'}
              </button>
            </div>
          )}
        </motion.div>
      </div>
    </AnimatePresence>
  );
}
