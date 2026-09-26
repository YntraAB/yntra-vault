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
  ShieldAlert,
  Lock,
  KeyRound,
  ArrowRight,
  ArrowLeft,
  QrCode,
  Camera,
  Fingerprint,
  HelpCircle,
} from 'lucide-react';
import { useAuth } from '@/features/auth';
import { useSettings, Toggle } from '@/features/settings';
import { useEntries } from '@/features/entries';
import { useToast } from '@/contexts/ToastContext';
import { useTranslation } from '@/contexts/LanguageContext';
import { ActionTooltip } from '@/components/ui/tooltip';
import type { PairingStats, QrSessionInfo } from '@/lib/backend';
import { formatIpv4Input } from '../utils/formatIpv4';
import { QrCodeView } from './QrCodeView';
import { QrScannerModal } from './QrScannerModal';
import { P2pVpnWarningModal, P2P_VPN_STORAGE_KEY } from './P2pVpnWarningModal';

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
    defaultRole === 'client' ? 'code' : defaultRole ? 'password' : 'role'
  );
  const [selectedRole, setSelectedRole] = useState<'host' | 'client'>(defaultRole || 'host');
  const [pairingMode, setPairingMode] = useState<'qr' | 'pin'>('qr');
  const [qrSession, setQrSession] = useState<QrSessionInfo | null>(null);
  const [isQrScannerOpen, setIsQrScannerOpen] = useState<boolean>(false);
  const [includePasswordChecked, setIncludePasswordChecked] = useState<boolean>(true);
  const [canEnrollBiometric, setCanEnrollBiometric] = useState<boolean>(false);
  const [clientSas, setClientSas] = useState<string | null>(null);
  const [isManualPasswordAdoption, setIsManualPasswordAdoption] = useState<boolean>(false);
  const [biometricAvailable, setBiometricAvailable] = useState<boolean>(false);
  const [biometricEnrolled, setBiometricEnrolled] = useState<boolean>(false);
  const [isEnrollingBiometric, setIsEnrollingBiometric] = useState<boolean>(false);
  const [isVpnWarningOpen, setIsVpnWarningOpen] = useState<boolean>(false);
  const [isInitialVpnWarning, setIsInitialVpnWarning] = useState<boolean>(false);
  const [prevIsOpen, setPrevIsOpen] = useState<boolean>(isOpen);

  if (isOpen !== prevIsOpen) {
    setPrevIsOpen(isOpen);
    if (isOpen) {
      let shouldShowVpn = false;
      try {
        shouldShowVpn = localStorage.getItem(P2P_VPN_STORAGE_KEY) !== 'true';
      } catch {
        // Ignore storage access errors
      }
      setIsVpnWarningOpen(shouldShowVpn);
      setIsInitialVpnWarning(shouldShowVpn);
    } else {
      setIsVpnWarningOpen(false);
      setIsInitialVpnWarning(false);
    }
  }

  const [pairingCode, setPairingCode] = useState<string>('');
  const [inputDigits, setInputDigits] = useState<string[]>(['', '', '', '', '', '']);
  const [password, setPassword] = useState<string>('');
  const [showPassword, setShowPassword] = useState<boolean>(false);
  const [copiedCode, setCopiedCode] = useState(false);
  const [errorMsg, setErrorMsg] = useState<string | null>(null);
  const [statusMsg, setStatusMsg] = useState<string>('');
  const [pairingStats, setPairingStats] = useState<PairingStats | null>(null);
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
  const attemptRef = useRef(0);
  const qrQueue = useRef<Promise<void>>(Promise.resolve());
  const [qrRevision, setQrRevision] = useState(0);
  const listenerToggleRef = useRef(toggleP2pListener);
  listenerToggleRef.current = toggleP2pListener;
  const qrView = useRef({ addToast, t });
  qrView.current = { addToast, t };
  const adoptedVaultPathRef = useRef<string | null>(null);

  const handleCopyIp = async (ipToCopy: string) => {
    if (!backend) return;
    try { await backend.copyToClipboard(`${ipToCopy.includes(':') ? `[${ipToCopy}]` : ipToCopy}:5324`, false); }
    catch { addToast({ message: t('toast.copy_failed'), type: 'error' }); return; }
    setCopiedIp(true);
    setTimeout(() => setCopiedIp(false), 1500);
  };

  const handleCopyCode = async () => {
    if (!pairingCode || !backend) return;
    try { await backend.copyToClipboard(pairingCode, true, 30); }
    catch { addToast({ message: t('toast.copy_failed'), type: 'error' }); return; }
    setCopiedCode(true);
    setTimeout(() => setCopiedCode(false), 1500);
  };

  // Check biometric availability
  useEffect(() => {
    if (backend && isOpen) {
      backend.checkBiometricAvailable().then((res) => {
        setBiometricAvailable(res?.available ?? false);
      }).catch(() => {});
    }
  }, [backend, isOpen]);

  // Reset state when opening & pause background listener to avoid port contention
  useEffect(() => {
    if (isOpen) {
      listenerToggleRef.current(false);
      attemptRef.current += 1;
      isCancelledRef.current = false;
      adoptedVaultPathRef.current = null;
      setErrorMsg(null);
      setPairingStats(null);
      setPassword('');
      setInputDigits(['', '', '', '', '', '']);
      setManualIp('');
      setShowManualIp(false);
      setPairingMode('qr');
      setQrSession(null);
      setIsQrScannerOpen(false);
      setIncludePasswordChecked(true);
      setCanEnrollBiometric(false);
      setClientSas(null);
      setIsManualPasswordAdoption(false);
      setBiometricEnrolled(false);
      setIsEnrollingBiometric(false);
      if (defaultRole === 'client') {
        setSelectedRole('client');
        setStep('code');
      } else if (defaultRole) {
        setSelectedRole(defaultRole);
        setStep('password');
      } else {
        setSelectedRole('host');
        setStep('role');
      }
    }
  }, [isOpen, defaultRole]);

  // Autofocus password input on step transition to password
  useEffect(() => {
    if (step === 'password' && isOpen) {
      setTimeout(() => {
        passwordInputRef.current?.focus();
      }, 60);
    }
  }, [step, isOpen]);

  // Autofocus first PIN input on step transition to code for client (in PIN mode)
  useEffect(() => {
    if (step === 'code' && selectedRole === 'client' && pairingMode === 'pin' && isOpen) {
      setTimeout(() => {
        inputRefs.current[0]?.focus();
      }, 60);
    }
  }, [step, selectedRole, pairingMode, isOpen]);

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
        }
      }).catch(() => {
        backend.getLocalIp().then((ip) => {
          if (ip) {
            setHostIps([ip]);
          }
        }).catch(() => {});
      });
    }
  }, [backend, isOpen]);

  // Pre-generate pairing code for host (in PIN mode)
  useEffect(() => {
    if ((step === 'password' || step === 'code') && selectedRole === 'host' && backend && !pairingCode) {
      backend.generatePairingCode().then((code) => {
        setPairingCode(code);
      }).catch((e) => {
        setErrorMsg(t('pairing.err_code_gen', { err: String(e) }));
      });
    }
  }, [step, selectedRole, backend, pairingCode, t]);

  const startQrHost = useCallback(() => setQrRevision((revision) => revision + 1), []);

  // Each effect owns one attempt. Serial preparation prevents a delayed cancel
  // from destroying the next session; disposed attempts cannot update the UI.
  useEffect(() => {
    if (!backend || !isOpen || step !== 'code' || selectedRole !== 'host' || pairingMode !== 'qr') return;
    let disposed = false;
    setQrSession(null);
    setErrorMsg(null);
    qrQueue.current = qrQueue.current.catch(() => {}).then(async () => {
      await backend.cancelQrPairingHost();
      if (disposed) return;
      const session = await backend.generateQrPairingSession();
      if (disposed) return;
      setQrSession(session);
      void backend.startQrPairingHost(password, includePasswordChecked).then((stats) => {
        if (disposed) return;
        setPairingStats(stats);
        if (stats.peer_addr) localStorage.setItem('yntra_last_peer_addr', stats.peer_addr);
        setStep('success');
        const { addToast, t } = qrView.current;
        addToast({ message: stats.peer_saved === false ? t('pairing.received_pending_password') : t('pairing.success_toast', { count: stats.total_entries }), type: 'success' });
      }).catch((error) => {
        if (!disposed) setErrorMsg(String(error?.message || error));
      });
    }).catch((error) => {
      if (!disposed) setErrorMsg(String(error?.message || error));
    });
    return () => {
      disposed = true;
      qrQueue.current = qrQueue.current.catch(() => {}).then(() => backend.cancelQrPairingHost()).catch(() => {});
    };
  }, [backend, isOpen, step, selectedRole, pairingMode, password, includePasswordChecked, qrRevision]);

  // Client QR Scanned Handler
  const handleQrScanned = useCallback(async (scannedPayload: string) => {
    setIsQrScannerOpen(false);
    if (!backend) return;
    setErrorMsg(null);
    setStep('connecting');
    setStatusMsg(t('pairing.connecting_qr') || 'Ansluter till datorn via QR-kod...');

    const attempt = ++attemptRef.current;
    isCancelledRef.current = false;
    try {

      await qrQueue.current.catch(() => {});
      if (isCancelledRef.current || attempt !== attemptRef.current) return;
      await backend.cancelPairingHost();
      if (isCancelledRef.current || attempt !== attemptRef.current) return;
      const res = await backend.startQrPairingClient(scannedPayload);
      if (isCancelledRef.current || attempt !== attemptRef.current) return;

      if (res.sas_code) {
        setClientSas(res.sas_code);
      }
      if (res.stats?.vault_path) {
        adoptedVaultPathRef.current = res.stats.vault_path;
      }
      setPairingStats(res.stats);

      if (res.needs_password) {
        setIsManualPasswordAdoption(true);
        setStep('password');
        setStatusMsg('');
      } else {
        setCanEnrollBiometric(res.has_master_password);
        setStep('success');
        addToast({
          message: t('pairing.success_toast', { count: res.stats.total_entries }),
          type: 'success',
        });
      }
    } catch (err: any) {
      if (isCancelledRef.current || attempt !== attemptRef.current) return;
      setErrorMsg(typeof err === 'string' ? err : err.message || (t('pairing.err_connect_failed') || 'Kunde inte ansluta via QR-kod'));
      setStep('code');
    }
  }, [backend, addToast, t]);

  // Complete adoption if host did not provision password in transit
  const handleSaveAdoptedVaultWithPassword = useCallback(async () => {
    if (!backend || !password.trim()) {
      setErrorMsg(t('pairing.err_enter_password') || 'Please enter Master Password.');
      return;
    }
    setErrorMsg(null);
    setStep('connecting');
    setStatusMsg(t('pairing.saving_adopted_vault') || 'Krypterar och sparar valvet lokalt...');

    const attempt = ++attemptRef.current;
    isCancelledRef.current = false;
    try {
      const stats = await backend.completeAdoptedVault(password);
      if (isCancelledRef.current || attempt !== attemptRef.current) return;
      setPairingStats(stats);
      setCanEnrollBiometric(true);
      setIsManualPasswordAdoption(false);
      setPassword('');
      setStep('success');
      addToast({
        message: t('pairing.success_toast', { count: stats.total_entries }),
        type: 'success',
      });
    } catch (err: any) {
      if (isCancelledRef.current || attempt !== attemptRef.current) return;
      setErrorMsg(typeof err === 'string' ? err : err.message || (t('pairing.err_failed') || 'Kunde inte spara valvet'));
      setStep('password');
    }
  }, [backend, password, addToast, t]);

  // Advance from Password step to Code step or submit manual password
  const handlePasswordAdvance = () => {
    if (!password.trim()) {
      setErrorMsg(t('pairing.err_enter_password') || 'Please enter Master Password.');
      return;
    }
    setErrorMsg(null);
    setStep('code');
  };

  const handlePasswordSubmit = () => {
    if (isManualPasswordAdoption) {
      handleSaveAdoptedVaultWithPassword();
    } else {
      handlePasswordAdvance();
    }
  };

  // Handle Host waiting for client (PIN mode)
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

    const attempt = ++attemptRef.current;
    isCancelledRef.current = false;
    try {

      await qrQueue.current.catch(() => {});
      if (isCancelledRef.current || attempt !== attemptRef.current) return;
      await backend.cancelPairingHost();
      if (isCancelledRef.current || attempt !== attemptRef.current) return;
      const listenAddr = '0.0.0.0:5324';
      const stats = await backend.startPairingHost(listenAddr, password, pairingCode);
      if (isCancelledRef.current || attempt !== attemptRef.current) return;
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
      if (isCancelledRef.current || attempt !== attemptRef.current) return;
      setErrorMsg(typeof err === 'string' ? err : err.message || (t('pairing.err_failed') || 'Pairing failed'));
      setStep('code');
    }
  }, [backend, pairingCode, password, addToast, t]);

  // Handle Client connecting to host (PIN mode)
  const startClientConnecting = useCallback(async () => {
    if (!backend) return;
    if (!password) {
      setErrorMsg(t('pairing.err_enter_password') || 'Please enter Master Password.');
      return;
    }
    const fullCode = inputDigits.join('');
    if (fullCode.length !== 6) {
      setErrorMsg(t('pairing.err_enter_pin') || 'Please enter the complete 6-digit code.');
      return;
    }

    setErrorMsg(null);
    setStep('connecting');

    const attempt = ++attemptRef.current;
    isCancelledRef.current = false;
    try {

      await qrQueue.current.catch(() => {});
      if (isCancelledRef.current || attempt !== attemptRef.current) return;
      await backend.cancelPairingHost();
      if (isCancelledRef.current || attempt !== attemptRef.current) return;
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
        if (isCancelledRef.current || attempt !== attemptRef.current) return;
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
      if (isCancelledRef.current || attempt !== attemptRef.current) return;

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
      if (isCancelledRef.current || attempt !== attemptRef.current) return;
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
    setErrorMsg(null);

    if (char && index < 5) {
      inputRefs.current[index + 1]?.focus();
    }
  };

  const handleDigitKeyDown = (index: number, e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'Backspace' && !inputDigits[index] && index > 0) {
      inputRefs.current[index - 1]?.focus();
    } else if (e.key === 'Enter') {
      const fullCode = inputDigits.join('');
      if (fullCode.length === 6) {
        startClientConnecting();
      }
    }
  };

  const handlePaste = (e: React.ClipboardEvent<HTMLInputElement>) => {
    e.preventDefault();
    const pasted = e.clipboardData.getData('text').replace(/\D/g, '').slice(0, 6);
    if (pasted.length > 0) {
      const next = ['', '', '', '', '', ''];
      for (let i = 0; i < pasted.length; i++) {
        next[i] = pasted[i];
      }
      setInputDigits(next);
      const focusIndex = Math.min(pasted.length, 5);
      inputRefs.current[focusIndex]?.focus();
    }
  };

  // Biometric Enrollment on Adopted Device
  const handleEnrollBiometric = async () => {
    if (!backend || !canEnrollBiometric) return;
    setIsEnrollingBiometric(true);
    try {
      await backend.enableBiometric();
      setBiometricEnrolled(true);
      setCanEnrollBiometric(false);
      addToast({
        message: t('pairing.biometric_enrolled_success') || 'Biometrisk inloggning aktiverad!',
        type: 'success',
      });
    } catch (err: any) {
      addToast({
        message: typeof err === 'string' ? err : err.message || 'Kunde inte aktivera biometri',
        type: 'error',
      });
    } finally {
      setIsEnrollingBiometric(false);
    }
  };

  // Memory cleanup and listener cancellation on unmount
  useEffect(() => {
    return () => {
      attemptRef.current += 1;
      isCancelledRef.current = true;
      setPassword('');
      setInputDigits(['', '', '', '', '', '']);
      setCanEnrollBiometric(false);
      setClientSas(null);
      setIsManualPasswordAdoption(false);
      qrQueue.current = qrQueue.current.catch(() => {}).then(() => backend?.cancelPairingHost()).catch(() => {});
      qrQueue.current = qrQueue.current.catch(() => {}).then(() => backend?.cancelQrPairingHost()).catch(() => {});
    };
  }, [backend]);

  const handleClose = () => {
    isCancelledRef.current = true;
    attemptRef.current += 1;
    qrQueue.current = qrQueue.current.catch(() => {}).then(() => backend?.cancelPairingHost()).catch(() => {});
    qrQueue.current = qrQueue.current.catch(() => {}).then(() => backend?.cancelQrPairingHost()).catch(() => {});
    setPassword('');
    setInputDigits(['', '', '', '', '', '']);
    setCanEnrollBiometric(false);
    setClientSas(null);
    setIsManualPasswordAdoption(false);
    if (settings.p2pAutoListen) {
      toggleP2pListener(true);
    }
    onClose();
  };

  const handleFinishSuccess = () => {
    const listenAfterPairing = autoListenChecked || (selectedRole === 'client' && autoSyncChecked);
    updateSettings({
      p2pAutoListen: listenAfterPairing,
      p2pAutoSyncWifi: autoSyncChecked,
    });
    setPassword('');
    setInputDigits(['', '', '', '', '', '']);
    setCanEnrollBiometric(false);
    setClientSas(null);
    setIsManualPasswordAdoption(false);
    if (listenAfterPairing) {
      toggleP2pListener(true);
    }
    if (onSuccess) onSuccess(pairingStats || undefined);
    onClose();
  };

  const isClientFlow = selectedRole === 'client' && !isManualPasswordAdoption;

  const stepTitles = isClientFlow
    ? [
        pairingMode === 'qr' ? (t('pairing.step_qr') || 'QR-kod') : (t('pairing.step_code') || 'PIN-kod'),
        t('pairing.step_sync') || 'Synkronisera',
      ]
    : [
        t('pairing.step_password') || 'Lösenord',
        pairingMode === 'qr' ? (t('pairing.step_qr') || 'QR-kod') : (t('pairing.step_code') || 'PIN-kod'),
        t('pairing.step_sync') || 'Synkronisera',
      ];

  const currentStepIdx = isClientFlow
    ? (step === 'code' ? 0 : 1)
    : (step === 'password' ? 0 : step === 'code' ? 1 : 2);

  return (
    <>
      <AnimatePresence>
        {isOpen && !(isVpnWarningOpen && isInitialVpnWarning) && (
          <motion.div
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            className="fixed inset-0 z-50 flex items-start sm:items-center justify-center overflow-y-auto bg-black/60 p-3 sm:p-4 touch-pan-y overscroll-contain"
            onClick={handleClose}
          >
            <motion.div
              initial={{ scale: 0.97, opacity: 0, y: 6 }}
              animate={{ scale: 1, opacity: 1, y: 0 }}
              exit={{ scale: 0.97, opacity: 0, y: 6 }}
              transition={{ duration: 0.15, ease: 'easeOut' }}
              className="w-full max-w-[430px] my-auto rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] shadow-xl overflow-hidden flex flex-col max-h-[calc(100dvh-1.5rem)] sm:max-h-[88vh]"
              onClick={(e) => e.stopPropagation()}
            >
              {/* Header */}
              <div className="flex items-center justify-between border-b border-[var(--border-subtle)] px-5 py-3.5 bg-[var(--bg-base)]">
                <div className="flex items-center gap-2.5">
                  <div className="flex h-7 w-7 items-center justify-center rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] text-[var(--text-secondary)]">
                    {step === 'password' ? (
                      <Lock size={14} />
                    ) : step === 'code' ? (
                      pairingMode === 'qr' ? <QrCode size={14} /> : <KeyRound size={14} />
                    ) : (
                      <Wifi size={14} />
                    )}
                  </div>
                  <div>
                    <h2 className="text-[14px] font-medium text-[var(--text-primary)] leading-tight">
                      {step === 'role' && (t('pairing.title') || 'Parkoppla enheter')}
                      {step === 'password' && (
                        isManualPasswordAdoption
                          ? (t('pairing.manual_adopt_title') || 'Ange master-lösenord')
                          : selectedRole === 'host'
                            ? (t('pairing.host_password_title') || 'Bekräfta master-lösenord')
                            : (t('pairing.client_password_title') || 'Ange valvlösenord')
                      )}
                      {step === 'code' && (selectedRole === 'host' ? (t('pairing.title_host') || 'Dela med QR eller PIN') : (t('pairing.title_client') || 'Anslut till dator'))}
                      {step === 'connecting' && (t('pairing.title_connecting') || 'Ansluter & synkroniserar...')}
                      {step === 'success' && (t('pairing.title_success') || 'Parkoppling klar')}
                    </h2>
                    <p className="text-[11px] text-[var(--text-tertiary)]">
                      {selectedRole === 'host' ? (t('pairing.role_host_badge') || 'Värddator • Wi-Fi P2P') : (t('pairing.role_client_badge') || 'Sekundär enhet • Wi-Fi P2P')}
                    </p>
                  </div>
                </div>
                <div className="flex items-center gap-1">
                  <ActionTooltip content={t('pairing.vpn_info_tooltip') || 'Viktigt om VPN'}>
                    <button
                      type="button"
                      onClick={() => {
                        setIsInitialVpnWarning(false);
                        setIsVpnWarningOpen(true);
                      }}
                      className="rounded-[3px] p-1 text-[var(--text-tertiary)] transition-colors hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)] cursor-pointer"
                    >
                      <ShieldAlert size={15} />
                    </button>
                  </ActionTooltip>
                  <button
                    type="button"
                    onClick={handleClose}
                    className="rounded-[3px] p-1 text-[var(--text-tertiary)] transition-colors hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)] cursor-pointer"
                  >
                    <X size={15} />
                  </button>
                </div>
              </div>

              {/* Stepper Progress Bar */}
              {step !== 'role' && (
                <div className="flex items-center justify-between gap-2 px-5 pt-3 pb-2 border-b border-[var(--border-subtle)] bg-[var(--bg-base)]/40">
                  {stepTitles.map((title, idx) => (
                    <div key={idx} className="flex flex-1 flex-col items-center gap-1.5 min-w-0">
                      <div
                        className={`h-1 w-full rounded-full transition-colors ${
                          idx <= currentStepIdx ? 'bg-[var(--text-primary)]' : 'bg-[var(--border-subtle)]'
                        }`}
                      />
                      <span title={title}
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
              <div className="flex flex-col gap-4 p-5 flex-1 min-h-0 overflow-y-auto touch-pan-y overscroll-contain">
                {/* Step 0: Role Selection */}
                {step === 'role' && (
                  <div className="flex flex-col gap-3">
                    <p className="text-[12px] text-[var(--text-secondary)]">
                      {t('pairing.select_role_desc') || 'Välj vilken roll denna enhet ska ha under parkopplingen:'}
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
                          <span>{t('pairing.role_host_title') || 'Dator (Värd)'}</span>
                          <ArrowRight size={13} className="text-[var(--text-tertiary)] group-hover:text-[var(--text-primary)] transition-colors" />
                        </div>
                        <div className="text-[11px] text-[var(--text-secondary)] mt-0.5 leading-snug">
                          {t('pairing.role_host_desc') || 'Denna dator håller ditt öppna valv och visar en QR-kod eller PIN-kod.'}
                        </div>
                      </div>
                    </button>

                    <button
                      type="button"
                      onClick={() => {
                        setSelectedRole('client');
                        // For client in adopt mode, can skip password directly to code/scan step
                        setStep('code');
                      }}
                      className="flex items-start gap-3 p-3.5 rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] hover:border-[var(--border-focus)] hover:bg-[var(--bg-hover)] transition-colors cursor-pointer text-left group"
                    >
                      <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] text-[var(--text-secondary)]">
                        <Smartphone size={18} />
                      </div>
                      <div className="flex-1 min-w-0">
                        <div className="text-[13px] font-medium text-[var(--text-primary)] flex items-center justify-between">
                          <span>{t('pairing.role_client_title') || 'Mobil / Sekundär enhet (Klient)'}</span>
                          <ArrowRight size={13} className="text-[var(--text-tertiary)] group-hover:text-[var(--text-primary)] transition-colors" />
                        </div>
                        <div className="text-[11px] text-[var(--text-secondary)] mt-0.5 leading-snug">
                          {t('pairing.role_client_desc') || 'Skanna QR-koden från din dator för att synkronisera valvet utan att skriva in lösenord.'}
                        </div>
                      </div>
                    </button>

                    <div className="flex justify-end pt-1">
                      <button
                        type="button"
                        onClick={handleClose}
                        className="h-8 rounded-[3px] px-3 text-[12px] font-medium text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] transition-colors cursor-pointer"
                      >
                        {t('common.cancel') || 'Avbryt'}
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
                        {isManualPasswordAdoption
                          ? (t('pairing.manual_adopt_password_desc') || 'Värddatorn inkluderade inte valvlösenordet i överföringen. Ange valvlösenordet för att kryptera och spara valvet på denna enhet.')
                          : selectedRole === 'host'
                            ? (t('pairing.host_password_desc') || 'Ange ditt master-lösenord för att bekräfta parkopplingen.')
                            : (t('pairing.client_password_desc') || 'Ange samma master-lösenord som används på värddatorn.')}
                      </span>
                    </div>

                    <div className="flex flex-col gap-1">
                      <div className="flex items-center gap-1.5">
                        <label className="text-[11px] font-medium text-[var(--text-secondary)]">
                          {t('pairing.master_password_label') || 'Master Password'}
                        </label>
                        <ActionTooltip
                          content={
                            isManualPasswordAdoption
                              ? (t('pairing.manual_adopt_password_desc') || 'Värddatorn inkluderade inte valvlösenordet i överföringen. Ange valvlösenordet för att kryptera och spara valvet på denna enhet.')
                              : selectedRole === 'host'
                                ? (t('pairing.host_password_desc') || 'Ange ditt master-lösenord för att bekräfta parkopplingen.')
                                : (t('pairing.client_password_desc') || 'Ange samma master-lösenord som används på värddatorn.')
                          }
                        >
                          <button
                            type="button"
                            className="text-[var(--text-tertiary)] hover:text-[var(--text-secondary)] transition-colors p-0.5 rounded cursor-pointer"
                          >
                            <HelpCircle size={12} />
                          </button>
                        </ActionTooltip>
                      </div>
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
                              handlePasswordSubmit();
                            }
                          }}
                          placeholder="••••••••••••"
                          className="h-9 w-full rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] px-3 pr-9 font-mono text-[12px] text-[var(--text-primary)] outline-none placeholder:font-sans placeholder:text-[var(--text-tertiary)] focus:border-[var(--border-focus)] transition-colors"
                        />
                        <ActionTooltip content={showPassword ? (t('login.hide_password') || 'Dölj lösenord') : (t('login.show_password') || 'Visa lösenord')}>
                          <button
                            type="button"
                            onClick={() => setShowPassword(!showPassword)}
                            className="absolute right-2 top-1/2 -translate-y-1/2 text-[var(--text-tertiary)] hover:text-[var(--text-primary)] p-1 cursor-pointer transition-colors"
                          >
                            {showPassword ? <EyeOff size={14} /> : <Eye size={14} />}
                          </button>
                        </ActionTooltip>
                      </div>
                      <span className="text-[10px] text-[var(--text-tertiary)] leading-tight mt-0.5">
                        {t('pairing.password_security_note') || 'Används endast i flyktigt minne för att upprätta en krypterad tunnel.'}
                      </span>
                    </div>

                    {errorMsg && (
                      <div className="rounded-[3px] border border-[var(--border)] bg-[var(--destructive)]/10 px-3 py-2 text-[11px] text-[var(--destructive)]">
                        {errorMsg}
                      </div>
                    )}

                    <div className="flex justify-between items-center pt-2">
                      {!defaultRole && !isManualPasswordAdoption ? (
                        <button
                          type="button"
                          onClick={() => setStep('role')}
                          className="flex items-center gap-1 h-8 rounded-[3px] px-2.5 text-[12px] font-medium text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] transition-colors cursor-pointer"
                        >
                          <ArrowLeft size={13} />
                          <span>{t('common.back') || 'Tillbaka'}</span>
                        </button>
                      ) : (
                        <button
                          type="button"
                          onClick={handleClose}
                          className="h-8 rounded-[3px] px-2.5 text-[12px] font-medium text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] transition-colors cursor-pointer"
                        >
                          {t('common.cancel') || 'Avbryt'}
                        </button>
                      )}

                      <button
                        type="button"
                        onClick={handlePasswordSubmit}
                        disabled={!password.trim()}
                        className="flex h-8 items-center gap-1.5 rounded-[3px] bg-[var(--text-primary)] px-3.5 text-[12px] font-medium text-[var(--bg-base)] transition-all hover:opacity-90 disabled:opacity-40 cursor-pointer shadow-xs"
                      >
                        <span>{isManualPasswordAdoption ? (t('pairing.save_vault_btn') || 'Spara & Öppna') : (t('pairing.next_btn') || 'Nästa')}</span>
                        <ArrowRight size={13} />
                      </button>
                    </div>
                  </div>
                )}

                {/* Step 2: Code & Pairing Execution Step */}
                {step === 'code' && (
                  <div className="flex flex-col gap-3.5">
                    {/* VPN Reminder banner */}
                    <button
                      type="button"
                      onClick={() => {
                        setIsInitialVpnWarning(false);
                        setIsVpnWarningOpen(true);
                      }}
                      className="flex items-center justify-between gap-2 px-2.5 py-1.5 rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] text-[11px] text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)] transition-colors cursor-pointer text-left w-full"
                    >
                      <span className="flex items-center gap-1.5 truncate">
                        <ShieldAlert size={13} className="shrink-0 text-[var(--text-tertiary)]" />
                        <span className="truncate">{t('pairing.vpn_banner_tip') || 'Tips: Stäng av aktiv VPN om enheterna inte hittas'}</span>
                      </span>
                      <span className="text-[10px] text-[var(--text-tertiary)] font-medium shrink-0 underline underline-offset-2">
                        {t('common.read_more') || 'Info'}
                      </span>
                    </button>

                    {/* Top Segmented Mode Selector: QR vs PIN */}
                    <div className="flex rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] p-0.5">
                      <button
                        type="button"
                        onClick={() => {
                          setPairingMode('qr');
                          setErrorMsg(null);
                        }}
                        className={`flex items-center justify-center gap-1.5 flex-1 py-1 px-2 text-[11px] font-medium rounded-[2px] transition-colors cursor-pointer ${
                          pairingMode === 'qr'
                            ? 'bg-[var(--bg-elevated)] text-[var(--text-primary)] shadow-xs'
                            : 'text-[var(--text-secondary)] hover:text-[var(--text-primary)]'
                        }`}
                      >
                        <QrCode size={13} />
                        <span>{t('pairing.mode_qr') || 'QR-kod (Snabbast)'}</span>
                      </button>
                      <button
                        type="button"
                        onClick={() => {
                          setPairingMode('pin');
                          setErrorMsg(null);
                          qrQueue.current = qrQueue.current.catch(() => {}).then(() => backend?.cancelQrPairingHost()).catch(() => {});
                        }}
                        className={`flex items-center justify-center gap-1.5 flex-1 py-1 px-2 text-[11px] font-medium rounded-[2px] transition-colors cursor-pointer ${
                          pairingMode === 'pin'
                            ? 'bg-[var(--bg-elevated)] text-[var(--text-primary)] shadow-xs'
                            : 'text-[var(--text-secondary)] hover:text-[var(--text-primary)]'
                        }`}
                      >
                        <KeyRound size={13} />
                        <span>{t('pairing.mode_pin') || '6-siffrig PIN'}</span>
                      </button>
                    </div>

                    {/* ─── Mode 1: QR Code Flow ─── */}
                    {pairingMode === 'qr' && (
                      <div>
                        {selectedRole === 'host' ? (
                          /* Host QR Display */
                          <div className="flex flex-col items-center gap-3">
                            {qrSession ? (
                              <QrCodeView
                                payload={qrSession.qr_payload}
                                sasCode={qrSession.sas_code}
                                expiresAt={qrSession.expires_at}
                                onRefresh={startQrHost}
                              />
                            ) : (
                              <div className="w-[190px] h-[190px] flex items-center justify-center">
                                <Loader2 size={24} className="animate-spin text-[var(--text-tertiary)]" />
                              </div>
                            )}

                            {/* Include Password Option */}
                            <div className="w-full flex items-center justify-between p-2.5 rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)]">
                              <div className="flex flex-col">
                                <span className="text-[11px] font-medium text-[var(--text-primary)]">
                                  {t('pairing.include_password_label') || 'Inkludera inloggning'}
                                </span>
                                <span className="text-[10px] text-[var(--text-secondary)]">
                                  {t('pairing.include_password_desc') || 'Överför lösenordet i den krypterade anslutningen'}
                                </span>
                              </div>
                              <Toggle
                                checked={includePasswordChecked}
                                onChange={(val) => {
                                  setIncludePasswordChecked(val);
                                  setQrSession(null);
                                }}
                              />
                            </div>

                            <p className="text-[10px] text-[var(--text-tertiary)] text-center max-w-xs leading-relaxed">
                              {t('pairing.qr_host_instruction') || 'Skanna koden med mobilens kamera för att synkronisera direkt över Wi-Fi.'}
                            </p>

                            <div className="flex justify-between items-center w-full pt-1">
                              <button
                                type="button"
                                onClick={() => {
                                  qrQueue.current = qrQueue.current.catch(() => {}).then(() => backend?.cancelQrPairingHost()).catch(() => {});
                                  setStep('password');
                                }}
                                className="flex items-center gap-1 h-8 rounded-[3px] px-2.5 text-[12px] font-medium text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] transition-colors cursor-pointer"
                              >
                                <ArrowLeft size={13} />
                                <span>{t('common.back') || 'Tillbaka'}</span>
                              </button>
                              <button
                                type="button"
                                onClick={handleClose}
                                className="h-8 rounded-[3px] px-3 text-[12px] font-medium text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] transition-colors cursor-pointer"
                              >
                                {t('common.cancel') || 'Avbryt'}
                              </button>
                            </div>
                          </div>
                        ) : (
                          /* Client QR Scanner Action Card */
                          <div className="flex flex-col items-center gap-3.5 py-1">
                            <div className="flex flex-col items-center text-center p-5 rounded-[4px] border border-[var(--border)] bg-[var(--bg-base)] w-full">
                              <div className="w-12 h-12 rounded-full border border-[var(--border)] bg-[var(--bg-elevated)] flex items-center justify-center text-[var(--text-primary)] mb-2.5 shadow-xs">
                                <Camera size={22} />
                              </div>
                              <h4 className="text-[13px] font-medium text-[var(--text-primary)] mb-1">
                                {t('pairing.scan_prompt_title') || 'Skanna QR-koden från datorn'}
                              </h4>
                              <p className="text-[11px] text-[var(--text-secondary)] max-w-xs leading-relaxed mb-4">
                                {t('pairing.scan_prompt_desc') || 'Rikta kameran mot QR-koden som visas på din datorskärm så kopplas valvet ihop automatiskt.'}
                              </p>
                              <button
                                type="button"
                                onClick={() => setIsQrScannerOpen(true)}
                                className="w-full flex items-center justify-center gap-2 h-9 rounded-[3px] bg-[var(--text-primary)] text-[var(--bg-base)] text-[12px] font-medium hover:opacity-90 transition-opacity cursor-pointer shadow-xs"
                              >
                                <Camera size={14} />
                                <span>{t('pairing.open_scanner_btn') || 'Öppna QR-skanner'}</span>
                              </button>
                            </div>

                            <div className="flex justify-between items-center w-full pt-1">
                              {!defaultRole ? (
                                <button
                                  type="button"
                                  onClick={() => setStep('role')}
                                  className="flex items-center gap-1 h-8 rounded-[3px] px-2.5 text-[12px] font-medium text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] transition-colors cursor-pointer"
                                >
                                  <ArrowLeft size={13} />
                                  <span>{t('common.back') || 'Tillbaka'}</span>
                                </button>
                              ) : <div />}
                              <button
                                type="button"
                                onClick={handleClose}
                                className="h-8 rounded-[3px] px-3 text-[12px] font-medium text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] transition-colors cursor-pointer"
                              >
                                {t('common.cancel') || 'Avbryt'}
                              </button>
                            </div>
                          </div>
                        )}
                      </div>
                    )}

                    {/* ─── Mode 2: Classic 6-Digit PIN Flow ─── */}
                    {pairingMode === 'pin' && (
                      <div>
                        {selectedRole === 'host' ? (
                          /* Host Display (PIN) */
                          <div className="flex flex-col gap-3">
                            <div className="flex flex-col items-center gap-2 py-1">
                              <span className="text-[10px] font-semibold uppercase tracking-wider text-[var(--text-tertiary)]">
                                {t('pairing.code_label') || 'Tillfällig PIN-kod'}
                              </span>
                              <div className="flex items-center gap-2">
                                <div className="flex h-12 items-center font-mono text-[24px] font-bold tracking-widest text-[var(--text-primary)] bg-[var(--bg-base)] px-6 rounded-[3px] border border-[var(--border)] shadow-xs">
                                  {pairingCode ? `${pairingCode.slice(0, 3)} ${pairingCode.slice(3)}` : '••••••'}
                                </div>
                                <button
                                  type="button"
                                  onClick={handleCopyCode}
                                  className="flex h-12 w-12 items-center justify-center rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:bg-[var(--bg-hover)] transition-colors cursor-pointer shadow-xs"
                                  title={t('pairing.copy_code') || 'Kopiera kod'}
                                >
                                  {copiedCode ? <Check size={16} className="text-[var(--text-primary)]" /> : <Copy size={16} />}
                                </button>
                              </div>

                              {hostIps.length > 0 && (
                                <div className="flex items-center justify-center gap-1.5 text-[11px] text-[var(--text-secondary)] bg-[var(--bg-base)] px-3 py-1 rounded-[3px] border border-[var(--border)] w-fit mx-auto mt-0.5">
                                  <span className="text-[var(--text-tertiary)]">{t('pairing.host_ip_label') || 'Host IP:'}</span>
                                  <span className="font-mono font-medium text-[var(--text-primary)]">{hostIps[0]}:5324</span>
                                  <button
                                    type="button"
                                    onClick={() => handleCopyIp(hostIps[0])}
                                    className="text-[var(--text-secondary)] hover:text-[var(--text-primary)] p-0.5 cursor-pointer ml-0.5"
                                    title={t('pairing.copy_ip') || 'Kopiera IP'}
                                  >
                                    {copiedIp ? <Check size={12} className="text-[var(--text-primary)]" /> : <Copy size={12} />}
                                  </button>
                                </div>
                              )}
                            </div>

                            <div className="flex items-start gap-2.5 rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] px-3 py-2 text-[11px] text-[var(--text-secondary)]">
                              <ShieldCheck size={14} className="mt-0.5 shrink-0 text-[var(--text-secondary)]" />
                              <span className="leading-relaxed">
                                {t('pairing.host_hint') || 'Skriv in denna 6-siffriga kod på din andra enhet för att parkoppla över Wi-Fi.'}
                              </span>
                            </div>

                            <div className="flex justify-between items-center pt-1">
                              <button
                                type="button"
                                onClick={() => setStep('password')}
                                className="flex items-center gap-1 h-8 rounded-[3px] px-2.5 text-[12px] font-medium text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] transition-colors cursor-pointer"
                              >
                                <ArrowLeft size={13} />
                                <span>{t('pairing.change_password') || 'Ändra lösenord'}</span>
                              </button>
                              <button
                                type="button"
                                onClick={startHostListening}
                                disabled={!password || !pairingCode}
                                className="flex h-8 items-center gap-1.5 rounded-[3px] bg-[var(--text-primary)] px-3.5 text-[12px] font-medium text-[var(--bg-base)] transition-all hover:opacity-90 disabled:opacity-40 cursor-pointer shadow-xs"
                              >
                                <Wifi size={13} />
                                <span>{t('pairing.listen_btn') || 'Börja lyssna & vänta'}</span>
                              </button>
                            </div>
                          </div>
                        ) : (
                          /* Client Input (PIN) */
                          <div className="flex flex-col gap-3">
                            <div className="flex flex-col gap-1.5">
                              <label className="text-[11px] font-medium text-[var(--text-secondary)] text-center">
                                {t('pairing.client_hint') || 'Ange den 6-siffriga koden som visas på datorn:'}
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

                            {/* Master Password Input for PIN mode */}
                            <div className="flex flex-col gap-1">
                              <div className="flex items-center gap-1.5">
                                <label className="text-[11px] font-medium text-[var(--text-secondary)]">
                                  {t('pairing.master_password_label') || 'Valvets Master Password'}
                                </label>
                                <ActionTooltip content={t('pairing.client_password_desc') || 'Ange samma master-lösenord som används på värddatorn för att auktorisera överföringen.'}>
                                  <button
                                    type="button"
                                    className="text-[var(--text-tertiary)] hover:text-[var(--text-secondary)] transition-colors p-0.5 rounded cursor-pointer"
                                  >
                                    <HelpCircle size={12} />
                                  </button>
                                </ActionTooltip>
                              </div>
                              <div className="relative">
                                <input
                                  type={showPassword ? 'text' : 'password'}
                                  value={password}
                                  onChange={(e) => {
                                    setPassword(e.target.value);
                                    if (errorMsg) setErrorMsg(null);
                                  }}
                                  onKeyDown={(e) => {
                                    if (e.key === 'Enter' && inputDigits.join('').length === 6 && password) {
                                      startClientConnecting();
                                    }
                                  }}
                                  placeholder="••••••••••••"
                                  className="h-9 w-full rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] px-3 pr-9 font-mono text-[12px] text-[var(--text-primary)] outline-none placeholder:font-sans placeholder:text-[var(--text-tertiary)] focus:border-[var(--border-focus)] transition-colors"
                                />
                                <ActionTooltip content={showPassword ? (t('login.hide_password') || 'Dölj lösenord') : (t('login.show_password') || 'Visa lösenord')}>
                                  <button
                                    type="button"
                                    onClick={() => setShowPassword(!showPassword)}
                                    className="absolute right-2 top-1/2 -translate-y-1/2 text-[var(--text-tertiary)] hover:text-[var(--text-primary)] p-1 cursor-pointer transition-colors"
                                  >
                                    {showPassword ? <EyeOff size={14} /> : <Eye size={14} />}
                                  </button>
                                </ActionTooltip>
                              </div>
                              <span className="text-[10px] text-[var(--text-tertiary)] leading-tight mt-0.5">
                                {t('pairing.pin_password_hint') || 'Krävs för att verifiera PIN-koden och kryptera överföringen.'}
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
                                  {showManualIp ? `− ${t('pairing.manual_ip_hide') || 'Dölj manuell IP'}` : `+ ${t('pairing.manual_ip_toggle') || 'Ange värddatorns IP manuellt'}`}
                                </span>
                              </button>
                              {showManualIp && (
                                <div className="flex flex-col gap-1 mt-0.5 p-2.5 rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)]">
                                  <input
                                    type="text"
                                    value={manualIp}
                                    onChange={(e) => setManualIp(formatIpv4Input(e.target.value, manualIp))}
                                    placeholder={t('pairing.manual_ip_ph') || 'T.ex. 192.168.1.12:5324'}
                                    className="h-7.5 w-full rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] px-2.5 font-mono text-[11px] text-[var(--text-primary)] outline-none placeholder:text-[var(--text-tertiary)] focus:border-[var(--border-focus)]"
                                  />
                                </div>
                              )}
                            </div>

                            <div className="flex justify-between items-center pt-1">
                              <button
                                type="button"
                                onClick={() => {
                                  if (!defaultRole) {
                                    setStep('role');
                                  } else {
                                    handleClose();
                                  }
                                }}
                                className="flex items-center gap-1 h-8 rounded-[3px] px-2.5 text-[12px] font-medium text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] transition-colors cursor-pointer"
                              >
                                <ArrowLeft size={13} />
                                <span>{!defaultRole ? (t('common.back') || 'Tillbaka') : (t('common.cancel') || 'Avbryt')}</span>
                              </button>
                              <button
                                type="button"
                                onClick={startClientConnecting}
                                disabled={!password || inputDigits.join('').length !== 6}
                                className="flex h-8 items-center gap-1.5 rounded-[3px] bg-[var(--text-primary)] px-3.5 text-[12px] font-medium text-[var(--bg-base)] transition-all hover:opacity-90 disabled:opacity-40 cursor-pointer shadow-xs"
                              >
                                <Wifi size={13} />
                                <span>{t('pairing.connect_btn') || 'Anslut & synka'}</span>
                              </button>
                            </div>
                          </div>
                        )}
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

                    {clientSas && (
                      <div className="flex items-center gap-1.5 px-3 py-1 rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] text-xs mx-auto">
                        <ShieldCheck className="w-3.5 h-3.5 text-[var(--text-secondary)]" />
                        <span className="text-[var(--text-tertiary)] font-mono">
                          {t('pairing.sas_label') || 'Bekräftelsekod:'}
                        </span>
                        <span className="font-mono font-bold tracking-widest text-[var(--text-primary)]">
                          {clientSas}
                        </span>
                        <Check size={13} className="text-emerald-500 ml-0.5" />
                      </div>
                    )}

                    <div className="text-[11px] text-[var(--text-secondary)] max-w-xs leading-relaxed">
                      {t('pairing.connecting_hint') || 'Håll båda enheterna vakna och anslutna till samma Wi-Fi-nätverk.'}
                    </div>

                    <button
                      type="button"
                      onClick={() => {
                        isCancelledRef.current = true;
                        attemptRef.current += 1;
                        qrQueue.current = qrQueue.current.catch(() => {}).then(() => backend?.cancelPairingHost()).catch(() => {});
                        qrQueue.current = qrQueue.current.catch(() => {}).then(() => backend?.cancelQrPairingHost()).catch(() => {});
                        setStep('code');
                      }}
                      className="mt-1 h-7.5 rounded-[3px] px-3 text-[11px] font-medium text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] transition-colors cursor-pointer border border-[var(--border)]"
                    >
                      {t('common.cancel') || 'Avbryt'}
                    </button>
                  </div>
                )}

                {/* Step 4: Success */}
                {step === 'success' && (
                  <div className="flex flex-col gap-3.5">
                    <div className="flex flex-col items-center justify-center text-center py-2 gap-1.5">
                      <div className="flex h-11 w-11 items-center justify-center rounded-full border border-[var(--border)] bg-[var(--bg-base)] text-[var(--text-primary)]">
                        <Check size={20} className="stroke-[2.5]" />
                      </div>
                      <h3 className="text-[14px] font-medium text-[var(--text-primary)]">
                          {pairingStats?.peer_saved === false ? t('pairing.received_title') : t('pairing.success_title')}
                      </h3>
                      <p className="text-[11px] text-[var(--text-secondary)]">
                          {pairingStats?.peer_saved === false ? t('pairing.received_pending_password') : t('pairing.success_desc', { count: pairingStats?.total_entries ?? 0 })}
                      </p>

                      {clientSas && (
                        <div className="flex items-center gap-1.5 px-3 py-1 rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] text-xs mx-auto mt-1">
                          <ShieldCheck className="w-3.5 h-3.5 text-[var(--text-secondary)]" />
                          <span className="text-[var(--text-tertiary)] font-mono">
                            {t('pairing.sas_label') || 'Bekräftelsekod:'}
                          </span>
                          <span className="font-mono font-bold tracking-widest text-[var(--text-primary)]">
                            {clientSas}
                          </span>
                          <Check size={13} className="text-emerald-500 ml-0.5" />
                        </div>
                      )}
                    </div>

                    {/* Biometric Enrollment Offer (if adopting on client with provisioned credentials) */}
                    {canEnrollBiometric && biometricAvailable && !biometricEnrolled && (
                      <div className="flex items-center justify-between p-3 rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)]">
                        <div className="flex items-center gap-2.5 min-w-0 flex-1">
                          <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] text-[var(--text-primary)]">
                            <Fingerprint size={16} />
                          </div>
                          <div className="min-w-0">
                            <div className="text-[12px] font-medium text-[var(--text-primary)] leading-tight">
                              {t('pairing.enroll_biometric_title') || 'Aktivera biometrisk inloggning'}
                            </div>
                            <div className="text-[10px] text-[var(--text-secondary)] truncate">
                              {t('pairing.enroll_biometric_desc') || 'Lås upp valvet på denna enhet i framtiden utan lösenord'}
                            </div>
                          </div>
                        </div>
                        <button
                          type="button"
                          disabled={isEnrollingBiometric}
                          onClick={handleEnrollBiometric}
                          className="flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium text-[var(--bg-base)] bg-[var(--text-primary)] hover:opacity-90 rounded-[3px] transition-all cursor-pointer shrink-0 ml-2"
                        >
                          {isEnrollingBiometric ? <Loader2 size={12} className="animate-spin" /> : <Check size={12} />}
                          <span>{t('pairing.enroll_biometric_btn') || 'Aktivera'}</span>
                        </button>
                      </div>
                    )}

                    {biometricEnrolled && (
                      <div className="flex items-center gap-2 p-2.5 rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] text-xs text-[var(--text-secondary)]">
                        <Check size={14} className="text-[var(--text-primary)]" />
                        <span>{t('pairing.biometric_enrolled_badge') || 'Biometrisk inloggning är nu aktiverad på denna enhet!'}</span>
                      </div>
                    )}

                    {/* Auto Sync Toggle */}
                    <div className="flex items-center justify-between gap-3 rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] p-3">
                      <div className="flex flex-col min-w-0 flex-1">
                        <span className="text-[11px] font-medium text-[var(--text-primary)]">
                          {selectedRole === 'host' ? (t('pairing.auto_listen_label') || 'Starta synkroniseringslyssnare automatiskt vid upplåsning') : (t('pairing.auto_sync_label') || 'Synkronisera automatiskt i bakgrunden över Wi-Fi')}
                        </span>
                        <span className="text-[10px] text-[var(--text-secondary)] leading-snug mt-0.5">
                          {selectedRole === 'host' ? (t('pairing.auto_listen_desc') || 'Rekommenderat för denna dator så mobilen kan ansluta i bakgrunden.') : (t('pairing.auto_sync_desc') || 'Rekommenderat för denna enhet så lösenord hålls uppdaterade utan manuella klick.')}
                        </span>
                      </div>
                      <Toggle
                        checked={selectedRole === 'host' ? autoListenChecked : autoSyncChecked}
                        onChange={(v) => {
                          if (selectedRole === 'host') setAutoListenChecked(v);
                          else { setAutoSyncChecked(v); setAutoListenChecked(v); }
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
                        {t('pairing.sync_again') || 'Synka igen'}
                      </button>
                      <button
                        type="button"
                        onClick={handleFinishSuccess}
                        className="flex h-8 items-center gap-1.5 rounded-[3px] bg-[var(--text-primary)] px-4 text-[12px] font-medium text-[var(--bg-base)] transition-all hover:opacity-90 cursor-pointer shadow-xs"
                      >
                        {t('common.done') || 'Klar'}
                      </button>
                    </div>
                  </div>
                )}
              </div>
            </motion.div>
          </motion.div>
        )}
      </AnimatePresence>

      {/* QR Scanner Camera Modal */}
      <QrScannerModal
        isOpen={isQrScannerOpen}
        onClose={() => setIsQrScannerOpen(false)}
        onScan={handleQrScanned}
      />

      {/* P2P VPN Warning Modal */}
      <P2pVpnWarningModal
        isOpen={isVpnWarningOpen}
        onClose={() => {
          if (isInitialVpnWarning) {
            handleClose();
          } else {
            setIsVpnWarningOpen(false);
          }
        }}
        onConfirm={() => {
          setIsVpnWarningOpen(false);
          setIsInitialVpnWarning(false);
        }}
      />
    </>
  );
};
