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
  Lock,
  KeyRound,
  ArrowRight,
  ArrowLeft,
} from 'lucide-react';
import { useAuth } from '@/features/auth';
import { useSettings, Toggle } from '@/features/settings';
import { useEntries } from '@/features/entries';
import { useToast } from '@/contexts/ToastContext';
import { useTranslation } from '@/contexts/LanguageContext';
import type { PairingStats } from '@/lib/backend';
import { formatIpv4Input } from '../utils/formatIpv4';

export interface DevicePairingWizardProps {
  isOpen: boolean;
  onClose: () => void;
  onSuccess?: (stats?: PairingStats) => void;
  defaultRole?: 'host' | 'client';
  isAdoptMode?: boolean;
}

type WizardStep = 'role' | 'password' | 'code' | 'connecting' | 'success';

export const DevicePairingWizard: React.FC<DevicePairingWizardProps> = ({
  isOpen,
  onClose,
  onSuccess,
  defaultRole,
  isAdoptMode,
}) => {
  const { backend, currentVault, isLocked } = useAuth();
  const { settings, updateSettings } = useSettings();
  const { toggleP2pListener } = useEntries();
  const { addToast } = useToast();
  const { t } = useTranslation();

  const [step, setStep] = useState<WizardStep>(
    defaultRole ? 'password' : 'role'
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
  const [hostIp, setHostIp] = useState<string | null>(null);
  const [hostIps, setHostIps] = useState<string[]>([]);
  const [copiedIp, setCopiedIp] = useState<boolean>(false);
  const [manualIp, setManualIp] = useState<string>('');
  const [showManualIp, setShowManualIp] = useState<boolean>(false);

  // Contextual startup choices
  const [autoListenChecked, setAutoListenChecked] = useState(settings.p2pAutoListen ?? true);
  const [autoSyncChecked, setAutoSyncChecked] = useState(settings.p2pAutoSyncWifi ?? true);

  const passwordInputRef = useRef<HTMLInputElement | null>(null);
  const inputRefs = useRef<(HTMLInputElement | null)[]>([]);
  const isCancelledRef = useRef(false);
  const adoptedVaultPathRef = useRef<string | null>(null);

  const handleCopyIp = (ipToCopy: string) => {
    navigator.clipboard.writeText(`${ipToCopy}:5324`);
    setCopiedIp(true);
    setTimeout(() => setCopiedIp(false), 1500);
  };

  // Reset state when opening & pause background listener to avoid port contention
  useEffect(() => {
    if (isOpen) {
      toggleP2pListener(false);
      isCancelledRef.current = false;
      adoptedVaultPathRef.current = null;
      setErrorMsg(null);
      setPairingStats(null);
      setPassword('');
      setInputDigits(['', '', '', '', '', '']);
      setManualIp('');
      setShowManualIp(false);
      if (defaultRole) {
        setSelectedRole(defaultRole);
        setStep('password');
      } else {
        setSelectedRole('host');
        setStep('role');
      }
    }
  }, [isOpen, defaultRole, toggleP2pListener]);

  // Autofocus password input on step transition to password
  useEffect(() => {
    if (step === 'password' && isOpen) {
      setTimeout(() => {
        passwordInputRef.current?.focus();
      }, 60);
    }
  }, [step, isOpen]);

  // Autofocus first PIN input on step transition to code for client
  useEffect(() => {
    if (step === 'code' && selectedRole === 'client' && isOpen) {
      setTimeout(() => {
        inputRefs.current[0]?.focus();
      }, 60);
    }
  }, [step, selectedRole, isOpen]);

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

  // Fetch host local IPs
  useEffect(() => {
    if (backend && isOpen) {
      backend.getLocalIps().then((ips) => {
        if (ips && ips.length > 0) {
          setHostIps(ips);
          setHostIp(ips[0]);
        }
      }).catch(() => {
        backend.getLocalIp().then((ip) => {
          if (ip) {
            setHostIp(ip);
            setHostIps([ip]);
          }
        }).catch(() => {});
      });
    }
  }, [backend, isOpen]);

  // Pre-generate pairing code for host
  useEffect(() => {
    if ((step === 'password' || step === 'code') && selectedRole === 'host' && backend && !pairingCode) {
      backend.generatePairingCode().then((code) => {
        setPairingCode(code);
      }).catch((e) => {
        setErrorMsg(t('pairing.err_code_gen', { err: String(e) }));
      });
    }
  }, [step, selectedRole, backend, pairingCode, t]);

  // Advance from Password step to Code step
  const handlePasswordAdvance = () => {
    if (!password.trim()) {
      setErrorMsg(t('pairing.err_enter_password') || 'Please enter Master Password.');
      return;
    }
    setErrorMsg(null);
    setStep('code');
  };

  // Handle Host waiting for client
  const startHostListening = useCallback(async () => {
    if (!backend || !pairingCode) return;
    if (!password) {
      setErrorMsg(t('pairing.err_enter_password') || 'Please enter Master Password.');
      setStep('password');
      return;
    }
    setErrorMsg(null);
    setStep('connecting');
    setStatusMsg(t('pairing.waiting_for_peer') || 'Waiting for the other device to connect over Wi-Fi...');

    try {
      const listenAddr = '0.0.0.0:5324';
      const stats = await backend.startPairingHost(listenAddr, password, pairingCode);
      if (isCancelledRef.current) return;
      setPairingStats(stats);
      if (stats.peer_addr) {
        const cleanPeerIp = stats.peer_addr.split(':')[0].trim();
        localStorage.setItem('yntra_last_peer_addr', cleanPeerIp);
      }
      setStep('success');
      addToast({
        message: t('pairing.success_toast', { count: stats.total_entries }),
        type: 'success',
      });
    } catch (err: any) {
      if (isCancelledRef.current) return;
      setErrorMsg(typeof err === 'string' ? err : err.message || (t('pairing.err_failed') || 'Pairing failed'));
      setStep('code');
    }
  }, [backend, pairingCode, password, addToast, t]);

  // Handle Client connecting to host
  const startClientConnecting = useCallback(async () => {
    if (!backend) return;
    if (!password) {
      setErrorMsg(t('pairing.err_enter_password') || 'Please enter Master Password.');
      setStep('password');
      return;
    }
    const fullCode = inputDigits.join('');
    if (fullCode.length !== 6) {
      setErrorMsg(t('pairing.err_enter_pin') || 'Please enter the complete 6-digit code.');
      return;
    }

    setErrorMsg(null);
    setStep('connecting');

    try {
      let targetAddr: string;

      if (manualIp.trim()) {
        const cleanIp = manualIp.trim();
        targetAddr = cleanIp.includes(':') ? cleanIp : `${cleanIp}:5324`;
        setStatusMsg(t('pairing.connecting_addr', { addr: targetAddr }) || `Connecting to ${targetAddr}...`);
      } else {
        setStatusMsg(t('pairing.scanning_beacon') || 'Scanning network for matching device...');
        const discovered = await backend.scanPairingDiscovery(password, fullCode, 10000);

        if (!discovered) {
          setShowManualIp(true);
          throw new Error(t('pairing.err_no_peer') || 'No device found on Wi-Fi with this code.');
        }
        targetAddr = discovered;
      }

      setStatusMsg(t('pairing.connecting_addr', { addr: targetAddr }) || `Connecting to ${targetAddr}...`);
      const isEffectiveAdopt = isAdoptMode || isLocked || !currentVault;
      const targetDbPath = adoptedVaultPathRef.current
        ? adoptedVaultPathRef.current
        : isEffectiveAdopt
          ? ''
          : (currentVault?.path || '');
      const stats = await backend.startPairingClient(targetAddr, password, fullCode, targetDbPath);
      if (isCancelledRef.current) return;

      if (stats?.vault_path) {
        adoptedVaultPathRef.current = stats.vault_path;
      }

      setPairingStats(stats);
      const rawAddr = stats.peer_addr || targetAddr;
      if (rawAddr) {
        const cleanPeerIp = rawAddr.split(':')[0].trim();
        localStorage.setItem('yntra_last_peer_addr', cleanPeerIp);
      }
      setStep('success');
      addToast({
        message: t('pairing.success_toast', { count: stats.total_entries }),
        type: 'success',
      });
    } catch (err: any) {
      if (isCancelledRef.current) return;
      setErrorMsg(typeof err === 'string' ? err : err.message || (t('pairing.err_connect_failed') || 'Could not connect to device'));
      setStep('code');
    }
  }, [backend, password, inputDigits, manualIp, currentVault, isAdoptMode, isLocked, addToast, t]);

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
      if (inputDigits.join('').length === 6) {
        startClientConnecting();
      }
    }
  };

  const handleCopyCode = () => {
    if (!pairingCode) return;
    navigator.clipboard.writeText(pairingCode);
    setCopiedCode(true);
    setTimeout(() => setCopiedCode(false), 1500);
  };

  // Memory cleanup and listener cancellation on unmount
  useEffect(() => {
    return () => {
      setPassword('');
      setInputDigits(['', '', '', '', '', '']);
      backend?.cancelPairingHost().catch(() => {});
    };
  }, [backend]);

  const handleClose = () => {
    isCancelledRef.current = true;
    backend?.cancelPairingHost().catch(() => {});
    setPassword('');
    setInputDigits(['', '', '', '', '', '']);
    if (settings.p2pAutoListen) {
      toggleP2pListener(true);
    }
    onClose();
  };

  const handleFinishSuccess = () => {
    updateSettings({
      p2pAutoListen: autoListenChecked,
      p2pAutoSyncWifi: autoSyncChecked,
    });
    setPassword('');
    setInputDigits(['', '', '', '', '', '']);
    if (autoListenChecked) {
      toggleP2pListener(true);
    }
    if (onSuccess) onSuccess(pairingStats || undefined);
    onClose();
  };

  const stepTitles = [
    t('pairing.step_password') || 'Password',
    t('pairing.step_code') || 'Pairing PIN',
    t('pairing.step_sync') || 'Synchronize',
  ];

  const currentStepIdx = step === 'password' ? 0 : step === 'code' ? 1 : 2;

  return (
    <AnimatePresence>
      {isOpen && (
        <motion.div
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 select-none p-4"
          onClick={handleClose}
        >
        <motion.div
          initial={{ scale: 0.97, opacity: 0, y: 6 }}
          animate={{ scale: 1, opacity: 1, y: 0 }}
          exit={{ scale: 0.97, opacity: 0, y: 6 }}
          transition={{ duration: 0.15, ease: 'easeOut' }}
          className="w-full max-w-[420px] rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] shadow-xl overflow-hidden flex flex-col"
          onClick={(e) => e.stopPropagation()}
        >
          {/* Header */}
          <div className="flex items-center justify-between border-b border-[var(--border-subtle)] px-5 py-3.5 bg-[var(--bg-base)]">
            <div className="flex items-center gap-2.5">
              <div className="flex h-7 w-7 items-center justify-center rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] text-[var(--text-secondary)]">
                {step === 'password' ? (
                  <Lock size={14} />
                ) : step === 'code' ? (
                  <KeyRound size={14} />
                ) : (
                  <Wifi size={14} />
                )}
              </div>
              <div>
                <h2 className="text-[14px] font-medium text-[var(--text-primary)] leading-tight">
                  {step === 'role' && (t('pairing.title') || 'Pair Devices')}
                  {step === 'password' && (selectedRole === 'host' ? (t('pairing.host_password_title') || 'Confirm Master Password') : (t('pairing.client_password_title') || 'Enter Vault Password'))}
                  {step === 'code' && (selectedRole === 'host' ? (t('pairing.title_host') || 'Share Pairing Code') : (t('pairing.title_client') || 'Enter Pairing PIN'))}
                  {step === 'connecting' && (t('pairing.title_connecting') || 'Connecting & Synchronizing...')}
                  {step === 'success' && (t('pairing.title_success') || 'Devices Paired Successfully')}
                </h2>
                <p className="text-[11px] text-[var(--text-tertiary)]">
                  {selectedRole === 'host' ? (t('pairing.role_host_badge') || 'Host Device • Wi-Fi P2P') : (t('pairing.role_client_badge') || 'Client Device • Wi-Fi P2P')}
                </p>
              </div>
            </div>
            <button
              type="button"
              onClick={handleClose}
              className="rounded-[3px] p-1 text-[var(--text-tertiary)] transition-colors hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)] cursor-pointer"
            >
              <X size={15} />
            </button>
          </div>

          {/* Stepper Progress Bar (Identical in style to Onboarding) */}
          {step !== 'role' && (
            <div className="flex items-center justify-between gap-1.5 px-5 pt-4 pb-1">
              {stepTitles.map((title, idx) => (
                <div key={idx} className="flex flex-1 flex-col items-center gap-1.5 min-w-0">
                  <div
                    className={`h-1 w-full rounded-full transition-colors ${
                      idx <= currentStepIdx ? 'bg-[var(--text-primary)]' : 'bg-[var(--border-subtle)]'
                    }`}
                  />
                  <span
                    className={`text-[10px] font-medium transition-colors whitespace-nowrap truncate ${
                      idx === currentStepIdx ? 'text-[var(--text-primary)]' : 'text-[var(--text-tertiary)]'
                    }`}
                  >
                    {title}
                  </span>
                </div>
              ))}
            </div>
          )}

          {/* Body Content */}
          <div className="flex flex-col gap-4 p-5">
            {/* Step 0: Role Selection */}
            {step === 'role' && (
              <div className="flex flex-col gap-3">
                <p className="text-[12px] text-[var(--text-secondary)]">
                  {t('pairing.select_role_desc') || 'Select the role for this device during the pairing process:'}
                </p>

                <button
                  type="button"
                  onClick={() => {
                    setSelectedRole('host');
                    setStep('password');
                  }}
                  className="flex items-start gap-3 p-3.5 rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] hover:border-[var(--border-focus)] hover:bg-[var(--bg-hover)] transition-colors cursor-pointer text-left group"
                >
                  <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] text-[var(--text-secondary)]">
                    <Laptop size={18} />
                  </div>
                  <div className="flex-1 min-w-0">
                    <div className="text-[13px] font-medium text-[var(--text-primary)] flex items-center justify-between">
                      <span>{t('pairing.role_host_title') || 'Computer (Host)'}</span>
                      <ArrowRight size={13} className="text-[var(--text-tertiary)] group-hover:text-[var(--text-primary)] transition-colors" />
                    </div>
                    <div className="text-[11px] text-[var(--text-secondary)] mt-0.5 leading-snug">
                      {t('pairing.role_host_desc') || 'This computer holds your vault and generates a 6-digit code for your other device.'}
                    </div>
                  </div>
                </button>

                <button
                  type="button"
                  onClick={() => {
                    setSelectedRole('client');
                    setStep('password');
                  }}
                  className="flex items-start gap-3 p-3.5 rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] hover:border-[var(--border-focus)] hover:bg-[var(--bg-hover)] transition-colors cursor-pointer text-left group"
                >
                  <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] text-[var(--text-secondary)]">
                    <Smartphone size={18} />
                  </div>
                  <div className="flex-1 min-w-0">
                    <div className="text-[13px] font-medium text-[var(--text-primary)] flex items-center justify-between">
                      <span>{t('pairing.role_client_title') || 'Phone / Secondary Device (Client)'}</span>
                      <ArrowRight size={13} className="text-[var(--text-tertiary)] group-hover:text-[var(--text-primary)] transition-colors" />
                    </div>
                    <div className="text-[11px] text-[var(--text-secondary)] mt-0.5 leading-snug">
                      {t('pairing.role_client_desc') || 'Enter the code displayed on your computer to receive and synchronize passwords.'}
                    </div>
                  </div>
                </button>

                <div className="flex justify-end pt-1">
                  <button
                    type="button"
                    onClick={handleClose}
                    className="h-8 rounded-[3px] px-3 text-[12px] font-medium text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] transition-colors cursor-pointer"
                  >
                    {t('common.cancel') || 'Cancel'}
                  </button>
                </div>
              </div>
            )}

            {/* Step 1: Master Password */}
            {step === 'password' && (
              <div className="flex flex-col gap-3.5">
                <div className="flex items-start gap-2.5 rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] p-3 text-[11px] text-[var(--text-secondary)] leading-relaxed">
                  <ShieldCheck size={15} className="mt-0.5 shrink-0 text-[var(--text-secondary)]" />
                  <span>
                    {selectedRole === 'host'
                      ? (t('pairing.host_password_desc') || 'Enter your master password to generate a secure, encrypted one-time pairing PIN.')
                      : (t('pairing.client_password_desc') || 'Enter the same master password used on your host computer to authorize synchronization.')}
                  </span>
                </div>

                <div className="flex flex-col gap-1">
                  <label className="text-[11px] font-medium text-[var(--text-secondary)]">
                    {t('pairing.master_password_label') || 'Master Password'}
                  </label>
                  <div className="relative">
                    <input
                      ref={passwordInputRef}
                      type={showPassword ? 'text' : 'password'}
                      value={password}
                      onChange={(e) => {
                        setPassword(e.target.value);
                        if (errorMsg) setErrorMsg(null);
                      }}
                      onKeyDown={(e) => {
                        if (e.key === 'Enter') {
                          handlePasswordAdvance();
                        }
                      }}
                      placeholder={t('pairing.master_password_ph') || 'Enter Master Password...'}
                      className="h-9 w-full rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] px-3 pr-9 text-[12px] text-[var(--text-primary)] outline-none placeholder:text-[var(--text-tertiary)] focus:border-[var(--border-focus)] transition-colors"
                    />
                    <button
                      type="button"
                      onClick={() => setShowPassword(!showPassword)}
                      className="absolute right-2 top-1/2 -translate-y-1/2 text-[var(--text-tertiary)] hover:text-[var(--text-primary)] p-1 cursor-pointer"
                      title={showPassword ? 'Hide password' : 'Show password'}
                    >
                      {showPassword ? <EyeOff size={14} /> : <Eye size={14} />}
                    </button>
                  </div>
                  <span className="text-[10px] text-[var(--text-tertiary)] leading-tight mt-0.5">
                    {t('pairing.password_security_note') || 'Used in volatile memory only to derive ephemeral pairing keys. Never sent over the network.'}
                  </span>
                </div>

                {errorMsg && (
                  <div className="rounded-[3px] border border-[var(--border)] bg-[var(--destructive)]/10 px-3 py-2 text-[11px] text-[var(--destructive)]">
                    {errorMsg}
                  </div>
                )}

                <div className="flex justify-between items-center pt-2">
                  {!defaultRole ? (
                    <button
                      type="button"
                      onClick={() => setStep('role')}
                      className="flex items-center gap-1 h-8 rounded-[3px] px-2.5 text-[12px] font-medium text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] transition-colors cursor-pointer"
                    >
                      <ArrowLeft size={13} />
                      <span>{t('common.back') || 'Back'}</span>
                    </button>
                  ) : (
                    <button
                      type="button"
                      onClick={handleClose}
                      className="h-8 rounded-[3px] px-2.5 text-[12px] font-medium text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] transition-colors cursor-pointer"
                    >
                      {t('common.cancel') || 'Cancel'}
                    </button>
                  )}

                  <button
                    type="button"
                    onClick={handlePasswordAdvance}
                    disabled={!password.trim()}
                    className="flex h-8 items-center gap-1.5 rounded-[3px] bg-[var(--text-primary)] px-3.5 text-[12px] font-medium text-[var(--bg-base)] transition-all hover:opacity-90 disabled:opacity-40 cursor-pointer shadow-xs"
                  >
                    <span>{t('pairing.next_btn') || 'Next'}</span>
                    <ArrowRight size={13} />
                  </button>
                </div>
              </div>
            )}

            {/* Step 2: Code Step */}
            {step === 'code' && (
              <div className="flex flex-col gap-3.5">
                {selectedRole === 'host' ? (
                  /* Host Display */
                  <div className="flex flex-col gap-3">
                    <div className="flex flex-col items-center gap-2 py-1">
                      <span className="text-[10px] font-semibold uppercase tracking-wider text-[var(--text-tertiary)]">
                        {t('pairing.code_label') || 'Temporary Pairing Code'}
                      </span>
                      <div className="flex items-center gap-2">
                        <div className="flex h-12 items-center font-mono text-[24px] font-bold tracking-widest text-[var(--text-primary)] bg-[var(--bg-base)] px-6 rounded-[3px] border border-[var(--border)] shadow-xs">
                          {pairingCode ? `${pairingCode.slice(0, 3)} ${pairingCode.slice(3)}` : '••••••'}
                        </div>
                        <button
                          type="button"
                          onClick={handleCopyCode}
                          className="flex h-12 w-12 items-center justify-center rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:bg-[var(--bg-hover)] transition-colors cursor-pointer shadow-xs"
                          title={t('pairing.copy_code') || 'Copy code'}
                        >
                          {copiedCode ? <Check size={16} className="text-[var(--text-primary)]" /> : <Copy size={16} />}
                        </button>
                      </div>

                      {/* IP Presentation */}
                      {hostIps.length > 0 ? (
                        <div className="flex flex-col items-center gap-1 mt-0.5 w-full">
                          <div className="flex items-center justify-center gap-1.5 text-[11px] text-[var(--text-secondary)] bg-[var(--bg-base)] px-3 py-1 rounded-[3px] border border-[var(--border)] w-fit mx-auto">
                            <span className="text-[var(--text-tertiary)]">{t('pairing.host_ip_label') || 'Host IP:'}</span>
                            <span className="font-mono font-medium text-[var(--text-primary)]">{hostIps[0]}:5324</span>
                            <button
                              type="button"
                              onClick={() => handleCopyIp(hostIps[0])}
                              className="text-[var(--text-secondary)] hover:text-[var(--text-primary)] p-0.5 cursor-pointer ml-0.5"
                              title={t('pairing.copy_ip') || 'Copy IP'}
                            >
                              {copiedIp ? <Check size={12} className="text-[var(--text-primary)]" /> : <Copy size={12} />}
                            </button>
                          </div>
                          {hostIps.length > 1 && (
                            <div className="text-[10px] text-[var(--text-tertiary)] flex items-center gap-1 flex-wrap justify-center">
                              <span>{t('pairing.other_networks') || 'Other networks:'}</span>
                              {hostIps.slice(1).map((altIp) => (
                                <button
                                  key={altIp}
                                  type="button"
                                  onClick={() => handleCopyIp(altIp)}
                                  className="font-mono bg-[var(--bg-base)] px-1.5 py-0.5 rounded-[2px] border border-[var(--border)] hover:border-[var(--border-focus)] transition-colors cursor-pointer text-[var(--text-secondary)] hover:text-[var(--text-primary)]"
                                  title={t('pairing.copy_ip') || 'Copy IP'}
                                >
                                  {altIp}
                                </button>
                              ))}
                            </div>
                          )}
                        </div>
                      ) : hostIp ? (
                        <div className="flex items-center justify-center gap-1.5 text-[11px] text-[var(--text-secondary)] bg-[var(--bg-base)] px-3 py-1 rounded-[3px] border border-[var(--border)] w-fit mx-auto mt-0.5">
                          <span className="text-[var(--text-tertiary)]">{t('pairing.host_ip_label') || 'Host IP:'}</span>
                          <span className="font-mono font-medium text-[var(--text-primary)]">{hostIp}:5324</span>
                          <button
                            type="button"
                            onClick={() => handleCopyIp(hostIp)}
                            className="text-[var(--text-secondary)] hover:text-[var(--text-primary)] p-0.5 cursor-pointer ml-0.5"
                            title={t('pairing.copy_ip') || 'Copy IP'}
                          >
                            {copiedIp ? <Check size={12} className="text-[var(--text-primary)]" /> : <Copy size={12} />}
                          </button>
                        </div>
                      ) : null}
                    </div>

                    <div className="flex items-start gap-2.5 rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] px-3 py-2 text-[11px] text-[var(--text-secondary)]">
                      <ShieldCheck size={14} className="mt-0.5 shrink-0 text-[var(--text-secondary)]" />
                      <span className="leading-relaxed">
                        {t('pairing.host_hint') || 'Enter this 6-digit code on your mobile or secondary device to pair vaults over Wi-Fi.'}
                      </span>
                    </div>

                    <p className="text-[10px] text-[var(--text-tertiary)] text-center">
                      {t('pairing.host_start_notice') || 'Click "Start Listening & Wait" below to open the secure local connection.'}
                    </p>

                    <div className="flex justify-between items-center pt-1">
                      <button
                        type="button"
                        onClick={() => setStep('password')}
                        className="flex items-center gap-1 h-8 rounded-[3px] px-2.5 text-[12px] font-medium text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] transition-colors cursor-pointer"
                      >
                        <ArrowLeft size={13} />
                        <span>{t('pairing.change_password') || 'Change password'}</span>
                      </button>
                      <button
                        type="button"
                        onClick={startHostListening}
                        disabled={!password || !pairingCode}
                        className="flex h-8 items-center gap-1.5 rounded-[3px] bg-[var(--text-primary)] px-3.5 text-[12px] font-medium text-[var(--bg-base)] transition-all hover:opacity-90 disabled:opacity-40 cursor-pointer shadow-xs"
                      >
                        <Wifi size={13} />
                        <span>{t('pairing.listen_btn') || 'Start Listening & Wait'}</span>
                      </button>
                    </div>
                  </div>
                ) : (
                  /* Client Input */
                  <div className="flex flex-col gap-3">
                    <div className="flex flex-col gap-1.5">
                      <label className="text-[11px] font-medium text-[var(--text-secondary)] text-center">
                        {t('pairing.client_hint') || 'Enter the 6-digit code shown on your computer:'}
                      </label>
                      <div className="flex justify-center gap-2 py-0.5">
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
                            className={`h-11 w-10 rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] text-center font-mono text-[18px] font-bold text-[var(--text-primary)] outline-none focus:border-[var(--border-focus)] transition-colors shadow-2xs ${idx === 2 ? 'mr-2' : ''}`}
                          />
                        ))}
                      </div>
                    </div>

                    <div className="flex items-start gap-2 rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] px-3 py-2 text-[11px] text-[var(--text-secondary)]">
                      <Wifi size={13} className="mt-0.5 shrink-0 text-[var(--text-secondary)]" />
                      <span className="leading-snug">
                        {t('pairing.client_wifi_hint') || 'Both devices will automatically discover each other on your local Wi-Fi.'}
                      </span>
                    </div>

                    {/* Manual IP Toggle */}
                    <div className="flex flex-col gap-1 pt-0.5">
                      <button
                        type="button"
                        onClick={() => setShowManualIp(!showManualIp)}
                        className="text-[11px] text-[var(--text-secondary)] hover:text-[var(--text-primary)] transition-colors text-left flex items-center gap-1 cursor-pointer w-fit"
                      >
                        <span className="underline underline-offset-2">
                          {showManualIp ? `− ${t('pairing.manual_ip_hide') || 'Hide manual IP'}` : `+ ${t('pairing.manual_ip_toggle') || 'Enter host IP manually'}`}
                        </span>
                      </button>
                      {showManualIp && (
                        <div className="flex flex-col gap-1 mt-0.5 p-2.5 rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)]">
                          <input
                            type="text"
                            value={manualIp}
                            onChange={(e) => setManualIp(formatIpv4Input(e.target.value, manualIp))}
                            placeholder={t('pairing.manual_ip_ph') || 'E.g. 192.168.1.12:5324'}
                            className="h-7.5 w-full rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] px-2.5 font-mono text-[11px] text-[var(--text-primary)] outline-none placeholder:text-[var(--text-tertiary)] focus:border-[var(--border-focus)]"
                          />
                          {localStorage.getItem('yntra_last_peer_addr') && !manualIp && (
                            <button
                              type="button"
                              onClick={() => setManualIp(localStorage.getItem('yntra_last_peer_addr') || '')}
                              className="text-[10px] text-[var(--text-secondary)] hover:text-[var(--text-primary)] self-start cursor-pointer underline underline-offset-2"
                            >
                              {t('pairing.last_connected_ip', { ip: localStorage.getItem('yntra_last_peer_addr') || '' }) || `Last connected IP: ${localStorage.getItem('yntra_last_peer_addr')}`}
                            </button>
                          )}
                          <span className="text-[9px] text-[var(--text-tertiary)] leading-tight">
                            {t('pairing.manual_ip_desc') || 'Use if your Wi-Fi router or mobile hotspot blocks local UDP broadcast discovery.'}
                          </span>
                        </div>
                      )}
                    </div>

                    <div className="flex justify-between items-center pt-1">
                      <button
                        type="button"
                        onClick={() => setStep('password')}
                        className="flex items-center gap-1 h-8 rounded-[3px] px-2.5 text-[12px] font-medium text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] transition-colors cursor-pointer"
                      >
                        <ArrowLeft size={13} />
                        <span>{t('pairing.change_password') || 'Change password'}</span>
                      </button>
                      <button
                        type="button"
                        onClick={startClientConnecting}
                        disabled={!password || inputDigits.join('').length !== 6}
                        className="flex h-8 items-center gap-1.5 rounded-[3px] bg-[var(--text-primary)] px-3.5 text-[12px] font-medium text-[var(--bg-base)] transition-all hover:opacity-90 disabled:opacity-40 cursor-pointer shadow-xs"
                      >
                        <Wifi size={13} />
                        <span>{t('pairing.connect_btn') || 'Connect & Sync'}</span>
                      </button>
                    </div>
                  </div>
                )}

                {errorMsg && (
                  <div className="rounded-[3px] border border-[var(--border)] bg-[var(--destructive)]/10 px-3 py-2 text-[11px] text-[var(--destructive)]">
                    {errorMsg}
                  </div>
                )}
              </div>
            )}

            {/* Step 3: Connecting */}
            {step === 'connecting' && (
              <div className="flex flex-col items-center justify-center py-4 gap-3 text-center">
                <div className="flex h-10 w-10 items-center justify-center rounded-full border border-[var(--border)] bg-[var(--bg-base)] text-[var(--text-primary)]">
                  <Loader2 size={20} className="animate-spin text-[var(--text-primary)]" />
                </div>

                <div className="text-[13px] font-medium text-[var(--text-primary)]">
                  {statusMsg}
                </div>

                {selectedRole === 'host' && (
                  <div className="flex flex-col items-center gap-1.5 my-0.5 p-3 bg-[var(--bg-base)] rounded-[3px] border border-[var(--border)] w-full">
                    <span className="text-[10px] font-semibold uppercase tracking-wider text-[var(--text-tertiary)]">
                      {t('pairing.code_label') || 'Temporary Pairing Code'}
                    </span>
                    <div className="flex h-10 items-center font-mono text-[20px] font-bold tracking-widest text-[var(--text-primary)] bg-[var(--bg-elevated)] px-5 rounded-[3px] border border-[var(--border)]">
                      {pairingCode ? `${pairingCode.slice(0, 3)} ${pairingCode.slice(3)}` : '••••••'}
                    </div>

                    {hostIps.length > 0 ? (
                      <div className="flex items-center justify-center gap-1 text-[11px] text-[var(--text-secondary)] mt-0.5">
                        <span className="text-[var(--text-tertiary)]">{t('pairing.host_ip_label') || 'Host IP:'}</span>
                        <span className="font-mono text-[var(--text-primary)]">{hostIps[0]}:5324</span>
                        <button
                          type="button"
                          onClick={() => handleCopyIp(hostIps[0])}
                          className="text-[var(--text-secondary)] hover:text-[var(--text-primary)] p-0.5 cursor-pointer ml-0.5"
                          title={t('pairing.copy_ip') || 'Copy IP'}
                        >
                          {copiedIp ? <Check size={11} className="text-[var(--text-primary)]" /> : <Copy size={11} />}
                        </button>
                      </div>
                    ) : null}

                    <span className="text-[10px] text-[var(--text-secondary)] font-medium mt-0.5 flex items-center gap-1.5">
                      <span className="w-1.5 h-1.5 rounded-full bg-[var(--text-primary)] animate-pulse" />
                      {t('pairing.host_listening_active') || 'Listening active – waiting for your other device...'}
                    </span>
                  </div>
                )}

                <div className="text-[11px] text-[var(--text-secondary)] max-w-xs leading-relaxed">
                  {t('pairing.connecting_hint') || 'Keep both devices awake and connected to the same Wi-Fi network.'}
                </div>

                <button
                  type="button"
                  onClick={() => {
                    isCancelledRef.current = true;
                    backend?.cancelPairingHost().catch(() => {});
                    setStep('code');
                  }}
                  className="mt-1 h-7.5 rounded-[3px] px-3 text-[11px] font-medium text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] transition-colors cursor-pointer border border-[var(--border)]"
                >
                  {t('common.cancel') || 'Cancel'}
                </button>
              </div>
            )}

            {/* Step 4: Success (Monochrome, subtle aesthetic) */}
            {step === 'success' && (
              <div className="flex flex-col gap-3.5">
                <div className="flex flex-col items-center justify-center text-center py-2 gap-1.5">
                  <div className="flex h-11 w-11 items-center justify-center rounded-full border border-[var(--border)] bg-[var(--bg-base)] text-[var(--text-primary)]">
                    <Check size={20} className="stroke-[2.5]" />
                  </div>
                  <h3 className="text-[14px] font-medium text-[var(--text-primary)]">
                    {t('pairing.success_title') || 'Devices Connected & Synchronized'}
                  </h3>
                  <p className="text-[11px] text-[var(--text-secondary)]">
                    {t('pairing.success_desc', { count: pairingStats?.total_entries ?? 0 }) || `${pairingStats?.total_entries ?? 0} passwords are now synchronized across both devices.`}
                  </p>
                </div>

                {/* Role-Specific Startup Automation Option with Toggle */}
                <div className="flex items-center justify-between gap-3 rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] p-3">
                  <div className="flex flex-col min-w-0 flex-1">
                    <span className="text-[11px] font-medium text-[var(--text-primary)]">
                      {selectedRole === 'host' ? (t('pairing.auto_listen_label') || 'Always start sync listener automatically on unlock') : (t('pairing.auto_sync_label') || 'Automatically sync in background on Wi-Fi')}
                    </span>
                    <span className="text-[10px] text-[var(--text-secondary)] leading-snug mt-0.5">
                      {selectedRole === 'host' ? (t('pairing.auto_listen_desc') || 'Recommended for this computer so your phone can connect in the background.') : (t('pairing.auto_sync_desc') || 'Recommended for this device so passwords stay updated without manual clicks.')}
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

                <div className="flex justify-between items-center pt-1">
                  <button
                    type="button"
                    onClick={() => {
                      setStep('code');
                    }}
                    className="h-8 rounded-[3px] px-2.5 text-[11px] font-medium text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] transition-colors cursor-pointer"
                  >
                    {t('pairing.sync_again') || 'Sync Again'}
                  </button>
                  <button
                    type="button"
                    onClick={handleFinishSuccess}
                    className="flex h-8 items-center gap-1.5 rounded-[3px] bg-[var(--text-primary)] px-4 text-[12px] font-medium text-[var(--bg-base)] transition-all hover:opacity-90 cursor-pointer shadow-xs"
                  >
                    {t('common.done') || 'Done'}
                  </button>
                </div>
              </div>
            )}
          </div>
        </motion.div>
      </motion.div>
      )}
    </AnimatePresence>
  );
};
