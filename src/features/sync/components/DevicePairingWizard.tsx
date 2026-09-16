import React, { useState, useEffect, useRef, useCallback } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import {
  Laptop,
  Smartphone,
  Wifi,
  Copy,
  Check,
  X,
  Loader2,
  Eye,
  EyeOff,
  ShieldCheck,
} from 'lucide-react';
import { useAuth } from '@/features/auth';
import { useSettings, Toggle } from '@/features/settings';
import { useToast } from '@/contexts/ToastContext';
import { useTranslation } from '@/contexts/LanguageContext';
import type { PairingStats } from '@/lib/backend';

export interface DevicePairingWizardProps {
  isOpen: boolean;
  onClose: () => void;
  onSuccess?: (stats?: PairingStats) => void;
  defaultRole?: 'host' | 'client';
}

type WizardStep = 'role' | 'host_display' | 'client_input' | 'connecting' | 'success';

export const DevicePairingWizard: React.FC<DevicePairingWizardProps> = ({
  isOpen,
  onClose,
  onSuccess,
  defaultRole,
}) => {
  const { backend, currentVault } = useAuth();
  const { settings, updateSettings } = useSettings();
  const { addToast } = useToast();
  const { t } = useTranslation();

  const [step, setStep] = useState<WizardStep>(
    defaultRole === 'client' ? 'client_input' : defaultRole === 'host' ? 'host_display' : 'role'
  );
  const [selectedRole, setSelectedRole] = useState<'host' | 'client'>(defaultRole || 'host');
  const [pairingCode, setPairingCode] = useState<string>('');
  const [inputDigits, setInputDigits] = useState<string[]>(['', '', '', '', '', '']);
  const [password, setPassword] = useState<string>('');
  const [showPassword, setShowPassword] = useState<boolean>(false);
  const [copiedCode, setCopiedCode] = useState(false);
  const [errorMsg, setErrorMsg] = useState<string | null>(null);
  const [statusMsg, setStatusMsg] = useState<string>('');
  const [pairingStats, setPairingStats] = useState<PairingStats | null>(null);

  // Contextual startup choices (only the active role is toggled)
  const [autoListenChecked, setAutoListenChecked] = useState(settings.p2pAutoListen ?? true);
  const [autoSyncChecked, setAutoSyncChecked] = useState(settings.p2pAutoSyncWifi ?? true);

  const inputRefs = useRef<(HTMLInputElement | null)[]>([]);
  const isCancelledRef = useRef(false);

  // Reset state when opening
  useEffect(() => {
    if (isOpen) {
      isCancelledRef.current = false;
      setErrorMsg(null);
      setPairingStats(null);
      setPassword('');
      setInputDigits(['', '', '', '', '', '']);
      if (defaultRole) {
        setSelectedRole(defaultRole);
        setStep(defaultRole === 'host' ? 'host_display' : 'client_input');
      } else {
        setSelectedRole('host');
        setStep('role');
      }
    }
  }, [isOpen, defaultRole]);

  // Handle ESC to close
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && isOpen) {
        handleClose();
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [isOpen]);

  // Generate pairing code when entering host_display
  useEffect(() => {
    if (step === 'host_display' && backend) {
      backend.generatePairingCode().then((code) => {
        setPairingCode(code);
      }).catch((e) => {
        setErrorMsg(t('pairing.err_code_gen', { err: String(e) }));
      });
    }
  }, [step, backend, t]);

  // Handle Host waiting for client
  const startHostListening = useCallback(async () => {
    if (!backend || !pairingCode) return;
    if (!password) {
      setErrorMsg(t('pairing.err_enter_password'));
      return;
    }
    setErrorMsg(null);
    setStep('connecting');
    setStatusMsg(t('pairing.waiting_for_peer'));

    try {
      const listenAddr = '0.0.0.0:5322';
      const stats = await backend.startPairingHost(listenAddr, password, pairingCode);
      if (isCancelledRef.current) return;
      setPairingStats(stats);
      setStep('success');
      addToast({
        message: t('pairing.success_toast', { count: stats.total_entries }),
        type: 'success',
      });
    } catch (err: any) {
      if (isCancelledRef.current) return;
      setErrorMsg(typeof err === 'string' ? err : err.message || t('pairing.err_failed'));
      setStep('host_display');
    }
  }, [backend, pairingCode, password, addToast, t]);

  // Handle Client connecting to host
  const startClientConnecting = useCallback(async () => {
    if (!backend) return;
    if (!password) {
      setErrorMsg(t('pairing.err_enter_password'));
      return;
    }
    const fullCode = inputDigits.join('');
    if (fullCode.length !== 6) {
      setErrorMsg(t('pairing.err_enter_pin'));
      return;
    }

    setErrorMsg(null);
    setStep('connecting');
    setStatusMsg(t('pairing.searching_wifi'));

    try {
      // Run auto-discovery scan for pairing beacon
      setStatusMsg(t('pairing.scanning_beacon'));
      const discovered = await backend.scanPairingDiscovery(password, fullCode, 4000);

      if (!discovered) {
        throw new Error(t('pairing.err_no_peer'));
      }

      setStatusMsg(t('pairing.connecting_addr', { addr: discovered }));
      const targetDbPath = currentVault?.path || '';
      const stats = await backend.startPairingClient(discovered, password, fullCode, targetDbPath);
      if (isCancelledRef.current) return;

      setPairingStats(stats);
      setStep('success');
      addToast({
        message: t('pairing.success_toast', { count: stats.total_entries }),
        type: 'success',
      });
    } catch (err: any) {
      if (isCancelledRef.current) return;
      setErrorMsg(typeof err === 'string' ? err : err.message || t('pairing.err_connect_failed'));
      setStep('client_input');
    }
  }, [backend, password, inputDigits, currentVault, addToast, t]);

  // Handle PIN input typing
  const handleDigitChange = (index: number, val: string) => {
    const char = val.slice(-1);
    if (!/^\d*$/.test(char)) return;

    const next = [...inputDigits];
    next[index] = char;
    setInputDigits(next);

    if (char && index < 5) {
      inputRefs.current[index + 1]?.focus();
    }
  };

  const handlePaste = (e: React.ClipboardEvent<HTMLInputElement>) => {
    e.preventDefault();
    const pasted = e.clipboardData.getData('text').replace(/\D/g, '').slice(0, 6);
    if (!pasted) return;
    const digits = pasted.split('');
    const next = [...inputDigits];
    for (let i = 0; i < 6; i++) {
      if (digits[i]) {
        next[i] = digits[i];
      }
    }
    setInputDigits(next);
    const nextFocus = Math.min(pasted.length, 5);
    inputRefs.current[nextFocus]?.focus();
  };

  const handleDigitKeyDown = (index: number, e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'Backspace' && !inputDigits[index] && index > 0) {
      inputRefs.current[index - 1]?.focus();
    } else if (e.key === 'Enter') {
      startClientConnecting();
    }
  };

  const handleCopyCode = () => {
    if (!pairingCode) return;
    navigator.clipboard.writeText(pairingCode);
    setCopiedCode(true);
    setTimeout(() => setCopiedCode(false), 2000);
  };

  // Memory cleanup on unmount
  useEffect(() => {
    return () => {
      setPassword('');
      setInputDigits(['', '', '', '', '', '']);
    };
  }, []);

  const handleClose = () => {
    isCancelledRef.current = true;
    setPassword('');
    setInputDigits(['', '', '', '', '', '']);
    onClose();
  };

  const handleFinishSuccess = () => {
    if (selectedRole === 'host') {
      updateSettings({
        p2pAutoListen: autoListenChecked,
      });
    } else {
      updateSettings({
        p2pAutoSyncWifi: autoSyncChecked,
      });
    }
    setPassword('');
    setInputDigits(['', '', '', '', '', '']);
    if (onSuccess) onSuccess(pairingStats || undefined);
    onClose();
  };

  if (!isOpen) return null;

  return (
    <AnimatePresence>
      <motion.div
        initial={{ opacity: 0 }}
        animate={{ opacity: 1 }}
        exit={{ opacity: 0 }}
        className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 select-none"
        onClick={handleClose}
      >
        <motion.div
          initial={{ scale: 0.96, opacity: 0 }}
          animate={{ scale: 1, opacity: 1 }}
          exit={{ scale: 0.96, opacity: 0 }}
          transition={{ duration: 0.15 }}
          className="w-full max-w-[460px] mx-3 rounded-lg border border-[var(--border)] bg-[var(--bg-base)] shadow-2xl overflow-hidden flex flex-col"
          onClick={(e) => e.stopPropagation()}
        >
          {/* Header */}
          <div className="flex items-center justify-between border-b border-[var(--border-subtle)] px-5 py-3.5">
            <div className="flex items-center gap-2.5">
              <Wifi size={18} className="text-[var(--text-primary)]" />
              <h2 className="text-[16px] font-semibold text-[var(--text-primary)]">
                {step === 'role' && t('pairing.title')}
                {step === 'host_display' && t('pairing.title_host')}
                {step === 'client_input' && t('pairing.title_client')}
                {step === 'connecting' && t('pairing.title_connecting')}
                {step === 'success' && t('pairing.title_success')}
              </h2>
            </div>
            <button
              type="button"
              onClick={handleClose}
              className="rounded-md p-1 text-[var(--text-tertiary)] transition-colors hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"
            >
              <X size={16} />
            </button>
          </div>

          {/* Body */}
          <div className="flex flex-col gap-4 p-5">
            {/* Step 1: Role Selection */}
            {step === 'role' && (
              <div className="flex flex-col gap-3">
                <p className="text-[13px] text-[var(--text-secondary)]">
                  {t('pairing.select_role_desc')}
                </p>

                <button
                  type="button"
                  onClick={() => {
                    setSelectedRole('host');
                    setStep('host_display');
                  }}
                  className="flex items-start gap-3.5 p-3.5 rounded-md border border-[var(--border)] bg-[var(--bg-elevated)] hover:border-[var(--border-focus)] hover:bg-[var(--bg-hover)] transition-all cursor-pointer text-left"
                >
                  <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-md border border-[var(--border)] bg-[var(--bg-base)] text-[var(--text-primary)]">
                    <Laptop size={18} />
                  </div>
                  <div>
                    <div className="text-[13px] font-semibold text-[var(--text-primary)]">
                      {t('pairing.role_host_title')}
                    </div>
                    <div className="text-[12px] text-[var(--text-secondary)] mt-0.5">
                      {t('pairing.role_host_desc')}
                    </div>
                  </div>
                </button>

                <button
                  type="button"
                  onClick={() => {
                    setSelectedRole('client');
                    setStep('client_input');
                  }}
                  className="flex items-start gap-3.5 p-3.5 rounded-md border border-[var(--border)] bg-[var(--bg-elevated)] hover:border-[var(--border-focus)] hover:bg-[var(--bg-hover)] transition-all cursor-pointer text-left"
                >
                  <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-md border border-[var(--border)] bg-[var(--bg-base)] text-[var(--text-primary)]">
                    <Smartphone size={18} />
                  </div>
                  <div>
                    <div className="text-[13px] font-semibold text-[var(--text-primary)]">
                      {t('pairing.role_client_title')}
                    </div>
                    <div className="text-[12px] text-[var(--text-secondary)] mt-0.5">
                      {t('pairing.role_client_desc')}
                    </div>
                  </div>
                </button>

                <div className="flex justify-end pt-2">
                  <button
                    type="button"
                    onClick={handleClose}
                    className="h-9 rounded-md px-4 text-[13px] font-medium text-[var(--text-secondary)] transition-colors hover:bg-[var(--bg-hover)]"
                  >
                    {t('common.cancel')}
                  </button>
                </div>
              </div>
            )}

            {/* Step 2A: Host Display */}
            {step === 'host_display' && (
              <div className="flex flex-col gap-4">
                <div className="flex flex-col gap-1.5">
                  <label className="text-[12px] font-medium text-[var(--text-secondary)]">
                    {t('pairing.master_password_label')}
                  </label>
                  <div className="relative">
                    <input
                      type={showPassword ? 'text' : 'password'}
                      value={password}
                      onChange={(e) => setPassword(e.target.value)}
                      onKeyDown={(e) => {
                        if (e.key === 'Enter' && password && pairingCode) startHostListening();
                      }}
                      placeholder={t('pairing.master_password_ph')}
                      className="h-9 w-full rounded-md border border-[var(--border)] bg-[var(--bg-elevated)] px-3 pr-10 text-[13px] text-[var(--text-primary)] outline-none placeholder:text-[var(--text-tertiary)] focus:border-[var(--border-focus)]"
                    />
                    <button
                      type="button"
                      onClick={() => setShowPassword(!showPassword)}
                      className="absolute right-2 top-1/2 -translate-y-1/2 text-[var(--text-tertiary)] hover:text-[var(--text-primary)] p-1 cursor-pointer"
                    >
                      {showPassword ? <EyeOff size={15} /> : <Eye size={15} />}
                    </button>
                  </div>
                </div>

                <div className="flex flex-col items-center gap-2 py-2">
                  <span className="text-[11px] font-medium uppercase tracking-wider text-[var(--text-tertiary)]">
                    {t('pairing.code_label')}
                  </span>
                  <div className="flex items-center gap-2">
                    <div className="flex h-12 items-center font-mono text-[24px] font-bold tracking-widest text-[var(--text-primary)] bg-[var(--bg-elevated)] px-6 rounded-md border border-[var(--border)] shadow-sm">
                      {pairingCode ? `${pairingCode.slice(0, 3)} ${pairingCode.slice(3)}` : '••••••'}
                    </div>
                    <button
                      type="button"
                      onClick={handleCopyCode}
                      className="flex h-12 w-12 items-center justify-center rounded-md border border-[var(--border)] bg-[var(--bg-elevated)] text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:bg-[var(--bg-hover)] transition-colors cursor-pointer shadow-sm"
                      title={t('common.copy')}
                    >
                      {copiedCode ? <Check size={16} className="text-emerald-500" /> : <Copy size={16} />}
                    </button>
                  </div>
                </div>

                <div className="flex items-start gap-2.5 rounded-md bg-[var(--bg-elevated)] px-3.5 py-2.5 text-[12px] text-[var(--text-secondary)]">
                  <ShieldCheck size={16} className="mt-0.5 shrink-0 text-[var(--text-primary)]" />
                  <span className="leading-relaxed">{t('pairing.host_hint')}</span>
                </div>

                {errorMsg && (
                  <div className="rounded-md bg-red-500/10 px-3 py-2 text-[12px] text-red-400">
                    {errorMsg}
                  </div>
                )}

                <div className="flex justify-between items-center pt-2">
                  <button
                    type="button"
                    onClick={() => setStep('role')}
                    className="h-9 rounded-md px-4 text-[13px] font-medium text-[var(--text-secondary)] transition-colors hover:bg-[var(--bg-hover)] cursor-pointer"
                  >
                    {t('common.back') || 'Back'}
                  </button>
                  <button
                    type="button"
                    onClick={startHostListening}
                    disabled={!password || !pairingCode}
                    className="flex h-9 items-center gap-2 rounded-md bg-[var(--text-primary)] px-4 text-[13px] font-semibold text-[var(--bg-base)] transition-all hover:opacity-90 disabled:opacity-50 cursor-pointer"
                  >
                    {t('pairing.listen_btn')}
                  </button>
                </div>
              </div>
            )}

            {/* Step 2B: Client Input */}
            {step === 'client_input' && (
              <div className="flex flex-col gap-4">
                <div className="flex flex-col gap-1.5">
                  <label className="text-[12px] font-medium text-[var(--text-secondary)]">
                    {t('pairing.master_password_label')}
                  </label>
                  <div className="relative">
                    <input
                      type={showPassword ? 'text' : 'password'}
                      value={password}
                      onChange={(e) => setPassword(e.target.value)}
                      onKeyDown={(e) => {
                        if (e.key === 'Enter') {
                          if (inputDigits.join('').length === 6) {
                            startClientConnecting();
                          } else {
                            inputRefs.current[0]?.focus();
                          }
                        }
                      }}
                      placeholder={t('pairing.master_password_ph')}
                      className="h-9 w-full rounded-md border border-[var(--border)] bg-[var(--bg-elevated)] px-3 pr-10 text-[13px] text-[var(--text-primary)] outline-none placeholder:text-[var(--text-tertiary)] focus:border-[var(--border-focus)]"
                    />
                    <button
                      type="button"
                      onClick={() => setShowPassword(!showPassword)}
                      className="absolute right-2 top-1/2 -translate-y-1/2 text-[var(--text-tertiary)] hover:text-[var(--text-primary)] p-1 cursor-pointer"
                    >
                      {showPassword ? <EyeOff size={15} /> : <Eye size={15} />}
                    </button>
                  </div>
                </div>

                <div className="flex flex-col gap-2">
                  <label className="text-[12px] font-medium text-[var(--text-secondary)] text-center">
                    {t('pairing.client_hint')}
                  </label>
                  <div className="flex justify-center gap-2 py-1">
                    {inputDigits.map((digit, idx) => (
                      <input
                        key={idx}
                        ref={(el) => { inputRefs.current[idx] = el; }}
                        type="text"
                        inputMode="numeric"
                        maxLength={1}
                        value={digit}
                        onChange={(e) => handleDigitChange(idx, e.target.value)}
                        onKeyDown={(e) => handleDigitKeyDown(idx, e)}
                        onPaste={handlePaste}
                        className={`h-12 w-11 rounded-md border border-[var(--border)] bg-[var(--bg-elevated)] text-center font-mono text-[20px] font-bold text-[var(--text-primary)] outline-none focus:border-[var(--border-focus)] transition-colors ${idx === 2 ? 'mr-3' : ''}`}
                      />
                    ))}
                  </div>
                </div>

                <div className="flex items-start gap-2.5 rounded-md bg-[var(--bg-elevated)] px-3.5 py-2.5 text-[12px] text-[var(--text-secondary)]">
                  <Wifi size={16} className="mt-0.5 shrink-0 text-[var(--text-primary)]" />
                  <span className="leading-relaxed">{t('pairing.client_wifi_hint')}</span>
                </div>

                {errorMsg && (
                  <div className="rounded-md bg-red-500/10 px-3 py-2 text-[12px] text-red-400">
                    {errorMsg}
                  </div>
                )}

                <div className="flex justify-between items-center pt-2">
                  <button
                    type="button"
                    onClick={() => setStep('role')}
                    className="h-9 rounded-md px-4 text-[13px] font-medium text-[var(--text-secondary)] transition-colors hover:bg-[var(--bg-hover)] cursor-pointer"
                  >
                    {t('common.back') || 'Back'}
                  </button>
                  <button
                    type="button"
                    onClick={startClientConnecting}
                    disabled={!password || inputDigits.join('').length !== 6}
                    className="flex h-9 items-center gap-2 rounded-md bg-[var(--text-primary)] px-4 text-[13px] font-semibold text-[var(--bg-base)] transition-all hover:opacity-90 disabled:opacity-50 cursor-pointer"
                  >
                    {t('pairing.connect_btn')}
                  </button>
                </div>
              </div>
            )}

            {/* Step 3: Connecting */}
            {step === 'connecting' && (
              <div className="flex flex-col items-center justify-center py-8 gap-3 text-center">
                <Loader2 size={32} className="animate-spin text-[var(--text-primary)]" />
                <div className="text-[14px] font-semibold text-[var(--text-primary)] mt-1">
                  {statusMsg}
                </div>
                <div className="text-[12px] text-[var(--text-secondary)] max-w-xs">
                  {t('pairing.connecting_hint')}
                </div>
              </div>
            )}

            {/* Step 4: Success */}
            {step === 'success' && (
              <div className="flex flex-col gap-4">
                <div className="flex flex-col items-center justify-center text-center py-2 gap-2">
                  <div className="flex h-12 w-12 items-center justify-center rounded-full bg-emerald-500/10 text-emerald-500">
                    <Check size={24} />
                  </div>
                  <h3 className="text-[15px] font-semibold text-[var(--text-primary)]">
                    {t('pairing.success_title')}
                  </h3>
                  <p className="text-[12px] text-[var(--text-secondary)]">
                    {t('pairing.success_desc', { count: pairingStats?.total_entries ?? 0 })}
                  </p>
                </div>

                {/* Role-Specific Startup Automation Option with Toggle */}
                <div className="flex items-center justify-between gap-4 rounded-md bg-[var(--bg-elevated)] p-3.5 border border-[var(--border)]">
                  <div className="flex flex-col min-w-0 flex-1">
                    <span className="text-[12px] font-semibold text-[var(--text-primary)]">
                      {selectedRole === 'host' ? t('pairing.auto_listen_label') : t('pairing.auto_sync_label')}
                    </span>
                    <span className="text-[11px] text-[var(--text-secondary)] leading-relaxed mt-0.5">
                      {selectedRole === 'host' ? t('pairing.auto_listen_desc') : t('pairing.auto_sync_desc')}
                    </span>
                  </div>
                  <Toggle
                    checked={selectedRole === 'host' ? autoListenChecked : autoSyncChecked}
                    onChange={(v) => {
                      if (selectedRole === 'host') setAutoListenChecked(v);
                      else setAutoSyncChecked(v);
                    }}
                  />
                </div>

                <div className="flex justify-end pt-2">
                  <button
                    type="button"
                    onClick={handleFinishSuccess}
                    className="flex h-9 items-center gap-2 rounded-md bg-[var(--text-primary)] px-5 text-[13px] font-semibold text-[var(--bg-base)] transition-all hover:opacity-90 cursor-pointer"
                  >
                    {t('common.confirm') || 'Done'}
                  </button>
                </div>
              </div>
            )}
          </div>
        </motion.div>
      </motion.div>
    </AnimatePresence>
  );
};
