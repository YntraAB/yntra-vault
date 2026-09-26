/**
 * ChangeMasterPasswordModal — Secure master password change
 */

import { useState, useEffect, useRef } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import {
  X,
  Loader2,
  ShieldCheck,
  KeyRound,
  FolderOpen,
  Eye,
  EyeOff,
  ArrowLeft,
  ArrowRight,
  Check,
} from 'lucide-react';
import { useToast } from '@/contexts/ToastContext';
import { useTranslation } from '@/contexts/LanguageContext';
import { isTauri, getBackend, openFileDialog, saveFileDialog, type StrengthScore, type StrengthLevel } from '@/lib/backend';
import { useBackend } from '@/lib/useBackend';
import { ActionTooltip } from '@/components/ui/tooltip';
import { SecureSecretInput, type SecureSecretInputRef } from '@/components/ui';

export interface ChangeMasterPasswordModalProps {
  open: boolean;
  onClose: () => void;
}

const STRENGTH_LEVELS: StrengthLevel[] = ['Critical', 'Weak', 'Fair', 'Strong', 'Excellent'];

export function ChangeMasterPasswordModal({ open, onClose }: ChangeMasterPasswordModalProps) {
  const { t } = useTranslation();
  const { addToast } = useToast();
  const { backend } = useBackend();

  // Wizard step: 0 = Current Password, 1 = New Password
  const [step, setStep] = useState<0 | 1>(0);

  const [currentPassword, setCurrentPassword] = useState('');
  const [newPassword, setNewPassword] = useState('');
  const [confirmPassword, setConfirmPassword] = useState('');
  const [showCurrentPassword, setShowCurrentPassword] = useState(false);
  const [showNewPassword, setShowNewPassword] = useState(false);
  const [showConfirmPassword, setShowConfirmPassword] = useState(false);
  const curInputRef = useRef<SecureSecretInputRef>(null);
  const newInputRef = useRef<SecureSecretInputRef>(null);
  const confirmInputRef = useRef<SecureSecretInputRef>(null);

  // Key File states
  const [useCurrentKeyFile, setUseCurrentKeyFile] = useState(false);
  const [currentKeyFile, setCurrentKeyFile] = useState('');
  const [useNewKeyFile, setUseNewKeyFile] = useState(false);
  const [newKeyFile, setNewKeyFile] = useState('');
  const [generateNewKeyFile, setGenerateNewKeyFile] = useState(false);

  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [strengthScore, setStrengthScore] = useState<StrengthScore | null>(null);

  useEffect(() => {
    if (!open) {
      setStep(0);
      setCurrentPassword('');
      setNewPassword('');
      setConfirmPassword('');
      setError(null);
      setShowCurrentPassword(false);
      setShowNewPassword(false);
      setShowConfirmPassword(false);
      setUseCurrentKeyFile(false);
      setCurrentKeyFile('');
      setUseNewKeyFile(false);
      setNewKeyFile('');
      setGenerateNewKeyFile(false);
      setStrengthScore(null);
    }
    return () => {
      setCurrentPassword('');
      setNewPassword('');
      setConfirmPassword('');
      setCurrentKeyFile('');
      setNewKeyFile('');
      setGenerateNewKeyFile(false);
      setStrengthScore(null);
    };
  }, [open]);

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && open) onClose();
    };
    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, [open, onClose]);

  // Autofocus input on step change
  useEffect(() => {
    if (!open) return;
    const timer = setTimeout(() => {
      if (step === 0) {
        curInputRef.current?.focus();
      } else {
        newInputRef.current?.focus();
      }
    }, 60);
    return () => clearTimeout(timer);
  }, [step, open]);

  // Asynchronously evaluate password strength without external calls
  useEffect(() => {
    if (!newPassword || !backend) {
      setStrengthScore(null);
      return;
    }
    let cancelled = false;
    const timer = setTimeout(async () => {
      try {
        const result = await backend.analyzePasswordStrength(newPassword);
        if (!cancelled && result) {
          setStrengthScore(result);
        }
      } catch {
        // Fallback
      }
    }, 120);
    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
  }, [newPassword, backend]);

  const handleBrowseCurrentKeyFile = async () => {
    if (!isTauri()) return;
    try {
      const selected = await openFileDialog({
        title: 'Select Current Key File',
        multiple: false,
        filters: [{ name: 'Key File (*.key, *.*)', extensions: ['key', '*'] }],
      });
      if (selected) {
        setCurrentKeyFile(typeof selected === 'string' ? selected : String(selected[0]));
      }
    } catch (e) {
      console.error('Browse current keyfile failed:', e);
    }
  };

  const handleBrowseNewKeyFile = async () => {
    if (!isTauri()) return;
    try {
      if (generateNewKeyFile) {
        const selected = await saveFileDialog({
          title: 'Save New Key File',
          defaultPath: 'vault.key',
          filters: [{ name: 'Key File (*.key)', extensions: ['key'] }],
        });
        if (selected) {
          setNewKeyFile(selected);
        }
      } else {
        const selected = await openFileDialog({
          title: 'Select Existing Key File',
          multiple: false,
          filters: [{ name: 'Key File (*.key, *.*)', extensions: ['key', '*'] }],
        });
        if (selected) {
          setNewKeyFile(typeof selected === 'string' ? selected : String(selected[0]));
        }
      }
    } catch (e) {
      console.error('Browse new keyfile failed:', e);
    }
  };

  // Step 0 -> Step 1 validation
  const handleAdvanceToStep1 = () => {
    setError(null);
    const curSecret = curInputRef.current?.getSecretBytes();
    const curBytes = curSecret && curSecret.length > 0 ? curSecret : new TextEncoder().encode(currentPassword);
    
    if (curBytes.length === 0) {
      setError(t('cmp.err_enter_current') || 'Enter your current password');
      return;
    }
    if (useCurrentKeyFile && !currentKeyFile.trim()) {
      setError(t('cmp.err_current_keyfile') || 'Please select current Key File');
      return;
    }

    setStep(1);
  };

  // Final Rekey Submission
  const handleSubmit = async (e?: React.FormEvent) => {
    if (e) e.preventDefault();
    setError(null);

    const curSecret = curInputRef.current?.getSecretBytes();
    const newSecret = newInputRef.current?.getSecretBytes();
    const curBytes = curSecret && curSecret.length > 0 ? curSecret : new TextEncoder().encode(currentPassword);
    const newBytes = newSecret && newSecret.length > 0 ? newSecret : new TextEncoder().encode(newPassword);

    if (curBytes.length === 0) {
      setStep(0);
      setError(t('cmp.err_enter_current') || 'Enter your current password');
      return;
    }
    if (useCurrentKeyFile && !currentKeyFile.trim()) {
      setStep(0);
      setError(t('cmp.err_current_keyfile') || 'Please select current Key File');
      return;
    }
    if (newBytes.length < 12) {
      setError(t('cmp.err_min_chars') || 'New password must be at least 12 characters');
      return;
    }
    if (!confirmPassword) {
      setError(t('cmp.err_confirm_pass') || 'Please confirm your new password');
      return;
    }
    if (newPassword !== confirmPassword) {
      setError(t('create_vault.err_pass_mismatch') || 'Passwords do not match');
      return;
    }
    if (useNewKeyFile && !newKeyFile.trim()) {
      setError(t('cmp.err_new_keyfile') || 'Please select or specify new Key File location');
      return;
    }

    setLoading(true);
    try {
      if (isTauri()) {
        const be = await getBackend();
        if (useNewKeyFile && generateNewKeyFile && newKeyFile.trim()) {
          await be.generateKeyFile(newKeyFile.trim());
        }

        await be.changeMasterPasswordBytes(
          curBytes,
          newBytes,
          useCurrentKeyFile && currentKeyFile.trim() ? currentKeyFile.trim() : undefined,
          useNewKeyFile && newKeyFile.trim() ? newKeyFile.trim() : undefined,
        );
      }
      addToast({ message: t('toast.master_password_changed') || 'Master password changed successfully', type: 'success' });
      onClose();
    } catch (err: any) {
      const errStr = err?.toString() || 'Failed to change password';
      setError(errStr);
      // If current password was wrong, return to Step 0 for correction
      if (errStr.toLowerCase().includes('password') || errStr.toLowerCase().includes('key') || errStr.toLowerCase().includes('invalid')) {
        setStep(0);
      }
    } finally {
      curBytes.fill(0);
      newBytes.fill(0);
      curInputRef.current?.clearSecretBytes();
      newInputRef.current?.clearSecretBytes();
      confirmInputRef.current?.clearSecretBytes();
      setLoading(false);
    }
  };

  const stepTitles = [
    t('cmp.step_current') || 'Current Password',
    t('cmp.step_new') || 'New Password',
  ];

  const currentLevelIdx = strengthScore
    ? STRENGTH_LEVELS.indexOf(strengthScore.level)
    : -1;

  return (
    <AnimatePresence>
      {open && (
        <motion.div
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          className="fixed inset-0 z-50 flex items-start sm:items-center justify-center overflow-y-auto bg-black/50 p-3 sm:p-4 touch-pan-y overscroll-contain"
          onClick={onClose}
        >
          <motion.div
            initial={{ scale: 0.97, opacity: 0, y: 6 }}
            animate={{ scale: 1, opacity: 1, y: 0 }}
            exit={{ scale: 0.97, opacity: 0, y: 6 }}
            transition={{ duration: 0.15, ease: 'easeOut' }}
            className="w-full max-w-[420px] my-auto rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] shadow-xl overflow-hidden flex flex-col max-h-[calc(100dvh-1.5rem)] sm:max-h-[85vh]"
            onClick={(e) => e.stopPropagation()}
          >
            {/* Window Header */}
            <div className="flex items-center justify-between border-b border-[var(--border-subtle)] px-5 py-3.5 bg-[var(--bg-base)]">
              <div className="flex items-center gap-2.5">
                <div className="flex h-7 w-7 items-center justify-center rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] text-[var(--text-secondary)]">
                  <KeyRound size={14} />
                </div>
                <div>
                  <h2 className="text-[14px] font-medium text-[var(--text-primary)] leading-tight">
                    {t('cmp.title')}
                  </h2>
                  <p className="text-[11px] text-[var(--text-tertiary)]">
                    {t('settings.master_password_desc')}
                  </p>
                </div>
              </div>
              <ActionTooltip content={t('common.close')}>
                <button
                  type="button"
                  onClick={onClose}
                  className="rounded-[3px] p-1 text-[var(--text-tertiary)] transition-colors hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)] cursor-pointer"
                >
                  <X size={15} />
                </button>
              </ActionTooltip>
            </div>

            {/* Stepper Progress Bar */}
            <div className="flex items-center justify-between gap-2 px-5 pt-3 pb-2 border-b border-[var(--border-subtle)] bg-[var(--bg-base)]/40">
              {stepTitles.map((title, idx) => (
                <div
                  key={idx}
                  onClick={() => {
                    if (idx < step) setStep(0);
                  }}
                  className={`flex flex-1 flex-col items-center gap-1.5 min-w-0 ${idx < step ? 'cursor-pointer' : ''}`}
                >
                  <div
                    className={`h-1 w-full rounded-full transition-colors ${
                      idx <= step ? 'bg-[var(--text-primary)]' : 'bg-[var(--border-subtle)]'
                    }`}
                  />
                  <span title={title}
                    className={`text-[10px] font-medium transition-colors whitespace-nowrap truncate ${
                      idx === step ? 'text-[var(--text-primary)]' : 'text-[var(--text-tertiary)]'
                    }`}
                  >
                    {title}
                  </span>
                </div>
              ))}
            </div>

            {/* Form Body with Animated Steps */}
            <div className="p-5 flex flex-col flex-1 min-h-0 overflow-y-auto touch-pan-y overscroll-contain">
              <AnimatePresence mode="wait">
                {/* STEP 0: CURRENT PASSWORD & CURRENT KEYFILE */}
                {step === 0 && (
                  <motion.div
                    key="step-0"
                    initial={{ opacity: 0, y: 6 }}
                    animate={{ opacity: 1, y: 0 }}
                    exit={{ opacity: 0, y: -6 }}
                    transition={{ duration: 0.15 }}
                    className="flex flex-col gap-3.5"
                  >
                    {/* Discrete Info Callout */}
                    <div className="flex items-start gap-2.5 rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] p-3 text-[11px] text-[var(--text-secondary)] leading-relaxed">
                      <KeyRound size={15} className="mt-0.5 shrink-0 text-[var(--text-secondary)]" />
                      <span>
                        {t('cmp.current_pass_desc') || 'Enter your current master password to verify authorization before re-encrypting the vault.'}
                      </span>
                    </div>

                    {/* Current Password Field */}
                    <div className="flex flex-col gap-1">
                      <label className="text-[11px] font-medium text-[var(--text-secondary)]">
                        {t('cmp.current_pass') || 'Current Password'}
                      </label>
                      <div className="relative">
                        <SecureSecretInput
                          ref={curInputRef}
                          value={currentPassword}
                          onChange={(v) => {
                            setCurrentPassword(v);
                            if (error) setError(null);
                          }}
                          onKeyDown={(e) => {
                            if (e.key === 'Enter') {
                              e.preventDefault();
                              handleAdvanceToStep1();
                            }
                          }}
                          show={showCurrentPassword}
                          placeholder={t('cmp.current_pass_ph') || 'Enter current password'}
                          className="h-9 w-full rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] px-3 pr-9 font-mono text-[12px] text-[var(--text-primary)] outline-none placeholder:font-sans placeholder:text-[var(--text-tertiary)] focus:border-[var(--border-focus)] transition-colors"
                        />
                        <ActionTooltip content={showCurrentPassword ? t('login.hide_password') : t('login.show_password')}>
                          <button
                            type="button"
                            onClick={() => setShowCurrentPassword(!showCurrentPassword)}
                            className="absolute right-2 top-1/2 -translate-y-1/2 text-[var(--text-tertiary)] hover:text-[var(--text-primary)] p-1 cursor-pointer transition-colors"
                          >
                            {showCurrentPassword ? <EyeOff size={14} /> : <Eye size={14} />}
                          </button>
                        </ActionTooltip>
                      </div>
                    </div>

                    {/* Current Key File Option */}
                    <div className="flex flex-col gap-2 rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] p-3">
                      <label className="flex items-center gap-2 cursor-pointer select-none text-[11px] font-medium text-[var(--text-secondary)] hover:text-[var(--text-primary)] transition-colors">
                        <input
                          type="checkbox"
                          checked={useCurrentKeyFile}
                          onChange={(e) => {
                            setUseCurrentKeyFile(e.target.checked);
                            if (error) setError(null);
                          }}
                          className="rounded-[2px] border-[var(--border)] accent-[var(--text-primary)]"
                        />
                        <KeyRound size={13} className="text-[var(--text-secondary)]" />
                        <span>{t('cmp.req_keyfile') || 'Vault currently requires Key File'}</span>
                      </label>
                      {useCurrentKeyFile && (
                        <div className="flex gap-1.5 pt-1">
                          <input
                            type="text"
                            value={currentKeyFile}
                            onChange={(e) => {
                              setCurrentKeyFile(e.target.value);
                              if (error) setError(null);
                            }}
                            placeholder={t('cmp.current_keyfile_ph') || 'Current .key file path'}
                            className="h-8 flex-1 rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] px-2.5 font-mono text-[11px] text-[var(--text-primary)] outline-none focus:border-[var(--border-focus)] placeholder:font-sans placeholder:text-[var(--text-tertiary)]"
                          />
                          {isTauri() && (
                            <ActionTooltip content={t('common.browse') || 'Browse'}>
                              <button
                                type="button"
                                onClick={handleBrowseCurrentKeyFile}
                                className="flex h-8 items-center gap-1.5 rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] px-2.5 text-[11px] font-medium text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)] transition-colors cursor-pointer shrink-0"
                              >
                                <FolderOpen size={12} />
                                <span>{t('common.browse') || 'Browse'}</span>
                              </button>
                            </ActionTooltip>
                          )}
                        </div>
                      )}
                    </div>

                    {/* Error Banner */}
                    {error && (
                      <div className="rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] px-3 py-2 text-[11px] text-[var(--destructive)] font-medium">
                        {error}
                      </div>
                    )}

                    {/* Step 0 Footer Actions */}
                    <div className="flex justify-between items-center pt-3 border-t border-[var(--border-subtle)] mt-2">
                      <button
                        type="button"
                        onClick={onClose}
                        className="h-8 rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] px-3 text-[12px] font-medium text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)] transition-colors cursor-pointer"
                      >
                        {t('common.cancel')}
                      </button>
                      <button
                        type="button"
                        onClick={handleAdvanceToStep1}
                        disabled={!currentPassword.trim()}
                        className="flex h-8 items-center gap-1.5 rounded-[3px] bg-[var(--text-primary)] px-3.5 text-[12px] font-medium text-[var(--bg-base)] transition-all hover:opacity-90 disabled:opacity-40 cursor-pointer shadow-xs"
                      >
                        <span>{t('common.next')}</span>
                        <ArrowRight size={13} />
                      </button>
                    </div>
                  </motion.div>
                )}

                {/* STEP 1: NEW PASSWORD, CONFIRM & NEW KEYFILE */}
                {step === 1 && (
                  <motion.div
                    key="step-1"
                    initial={{ opacity: 0, y: 6 }}
                    animate={{ opacity: 1, y: 0 }}
                    exit={{ opacity: 0, y: -6 }}
                    transition={{ duration: 0.15 }}
                    className="flex flex-col gap-3.5"
                  >
                    {/* Discrete Info Callout */}
                    <div className="flex items-start gap-2.5 rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] p-3 text-[11px] text-[var(--text-secondary)] leading-relaxed">
                      <ShieldCheck size={15} className="mt-0.5 shrink-0 text-[var(--text-secondary)]" />
                      <span>
                        {t('cmp.new_pass_desc') || 'Choose a strong master password of at least 12 characters. The vault will be re-encrypted with a new key.'}
                      </span>
                    </div>

                    {/* New Password Field */}
                    <div className="flex flex-col gap-1">
                      <label className="text-[11px] font-medium text-[var(--text-secondary)]">
                        {t('cmp.new_pass') || 'New Password'}
                      </label>
                      <div className="relative">
                        <SecureSecretInput
                          ref={newInputRef}
                          value={newPassword}
                          onChange={(v) => {
                            setNewPassword(v);
                            if (error) setError(null);
                          }}
                          onKeyDown={(e) => {
                            if (e.key === 'Enter') {
                              e.preventDefault();
                              confirmInputRef.current?.focus();
                            }
                          }}
                          show={showNewPassword}
                          placeholder={t('cmp.min_chars') || 'Minimum 12 characters'}
                          className="h-9 w-full rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] px-3 pr-9 font-mono text-[12px] text-[var(--text-primary)] outline-none placeholder:font-sans placeholder:text-[var(--text-tertiary)] focus:border-[var(--border-focus)] transition-colors"
                        />
                        <ActionTooltip content={showNewPassword ? t('login.hide_password') : t('login.show_password')}>
                          <button
                            type="button"
                            onClick={() => setShowNewPassword(!showNewPassword)}
                            className="absolute right-2 top-1/2 -translate-y-1/2 text-[var(--text-tertiary)] hover:text-[var(--text-primary)] p-1 cursor-pointer transition-colors"
                          >
                            {showNewPassword ? <EyeOff size={14} /> : <Eye size={14} />}
                          </button>
                        </ActionTooltip>
                      </div>
                    </div>

                    {/* Discrete Monochrome Strength Meter (No Colorful Badges) */}
                    {newPassword && strengthScore && (
                      <div className="flex flex-col gap-1.5 select-none py-0.5">
                        <div className="flex items-center gap-1">
                          {STRENGTH_LEVELS.map((lvl, idx) => (
                            <div
                              key={lvl}
                              className={`h-1 flex-1 rounded-full transition-colors ${
                                idx <= currentLevelIdx ? 'bg-[var(--text-primary)]' : 'bg-[var(--border-subtle)]'
                              }`}
                            />
                          ))}
                        </div>
                        <div className="flex items-center justify-between text-[10px] text-[var(--text-tertiary)]">
                          <span className="font-medium text-[var(--text-secondary)]">
                            {t(`strength.${strengthScore.level.toLowerCase()}`) || strengthScore.level}
                          </span>
                          <span>
                            {t('strength.bits_entropy', { bits: strengthScore.entropy_bits.toFixed(0) }) || `${strengthScore.entropy_bits.toFixed(0)} bits`}
                            {strengthScore.crack_time ? ` • ${strengthScore.crack_time}` : ''}
                          </span>
                        </div>
                      </div>
                    )}

                    {/* Confirm New Password Field */}
                    <div className="flex flex-col gap-1">
                      <label className="text-[11px] font-medium text-[var(--text-secondary)]">
                        {t('cmp.confirm_new_pass') || 'Confirm New Password'}
                      </label>
                      <div className="relative">
                        <SecureSecretInput
                          ref={confirmInputRef}
                          value={confirmPassword}
                          onChange={(v) => {
                            setConfirmPassword(v);
                            if (error) setError(null);
                          }}
                          onKeyDown={(e) => {
                            if (e.key === 'Enter') {
                              e.preventDefault();
                              handleSubmit();
                            }
                          }}
                          show={showConfirmPassword}
                          placeholder={t('cmp.reenter_pass_ph') || 'Re-enter new password'}
                          mismatch={!!confirmPassword && confirmPassword !== newPassword}
                          className="h-9 w-full rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] px-3 pr-9 font-mono text-[12px] text-[var(--text-primary)] outline-none placeholder:font-sans placeholder:text-[var(--text-tertiary)] focus:border-[var(--border-focus)] transition-colors"
                        />
                        <ActionTooltip content={showConfirmPassword ? t('login.hide_password') : t('login.show_password')}>
                          <button
                            type="button"
                            onClick={() => setShowConfirmPassword(!showConfirmPassword)}
                            className="absolute right-2 top-1/2 -translate-y-1/2 text-[var(--text-tertiary)] hover:text-[var(--text-primary)] p-1 cursor-pointer transition-colors"
                          >
                            {showConfirmPassword ? <EyeOff size={14} /> : <Eye size={14} />}
                          </button>
                        </ActionTooltip>
                      </div>
                      {confirmPassword && confirmPassword !== newPassword && (
                        <span className="text-[10px] text-[var(--destructive)]">
                          {t('create_vault.err_pass_mismatch') || 'Passwords do not match'}
                        </span>
                      )}
                    </div>

                    {/* New Key File Card */}
                    <div className="flex flex-col gap-2 rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] p-3">
                      <label className="flex items-center gap-2 cursor-pointer select-none text-[11px] font-medium text-[var(--text-secondary)] hover:text-[var(--text-primary)] transition-colors">
                        <input
                          type="checkbox"
                          checked={useNewKeyFile}
                          onChange={(e) => {
                            setUseNewKeyFile(e.target.checked);
                            if (error) setError(null);
                          }}
                          className="rounded-[2px] border-[var(--border)] accent-[var(--text-primary)]"
                        />
                        <KeyRound size={13} className="text-[var(--text-secondary)]" />
                        <span>{t('cmp.req_new_keyfile') || 'Require Key File for new key'}</span>
                      </label>

                      {useNewKeyFile && (
                        <div className="mt-1 flex flex-col gap-2 pl-5">
                          <div className="flex items-center gap-4 text-[11px] text-[var(--text-secondary)]">
                            <label className="flex items-center gap-1.5 cursor-pointer">
                              <input
                                type="radio"
                                name="newKeyFileMode"
                                checked={!generateNewKeyFile}
                                onChange={() => setGenerateNewKeyFile(false)}
                                className="accent-[var(--text-primary)]"
                              />
                              <span>{t('create_vault.use_existing_keyfile') || 'Use existing'}</span>
                            </label>
                            <label className="flex items-center gap-1.5 cursor-pointer">
                              <input
                                type="radio"
                                name="newKeyFileMode"
                                checked={generateNewKeyFile}
                                onChange={() => setGenerateNewKeyFile(true)}
                                className="accent-[var(--text-primary)]"
                              />
                              <span>{t('create_vault.gen_new_keyfile') || 'Generate new'}</span>
                            </label>
                          </div>

                          <div className="flex gap-1.5">
                            <input
                              type="text"
                              value={newKeyFile}
                              onChange={(e) => {
                                setNewKeyFile(e.target.value);
                                if (error) setError(null);
                              }}
                              placeholder={
                                generateNewKeyFile
                                  ? t('create_vault.save_keyfile_ph') || 'Save location for .key file'
                                  : t('cmp.new_keyfile_ph') || t('create_vault.exist_keyfile_ph') || 'New .key file path'
                              }
                              className="h-8 flex-1 rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] px-2.5 font-mono text-[11px] text-[var(--text-primary)] outline-none focus:border-[var(--border-focus)] placeholder:font-sans placeholder:text-[var(--text-tertiary)]"
                            />
                            {isTauri() && (
                              <ActionTooltip content={t('common.browse') || 'Browse'}>
                                <button
                                  type="button"
                                  onClick={handleBrowseNewKeyFile}
                                  className="flex h-8 items-center gap-1.5 rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] px-2.5 text-[11px] font-medium text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)] transition-colors cursor-pointer shrink-0"
                                >
                                  <FolderOpen size={12} />
                                  <span>{t('common.browse') || 'Browse'}</span>
                                </button>
                              </ActionTooltip>
                            )}
                          </div>
                        </div>
                      )}
                    </div>

                    {/* Error Banner */}
                    {error && (
                      <div className="rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] px-3 py-2 text-[11px] text-[var(--destructive)] font-medium">
                        {error}
                      </div>
                    )}

                    {/* Step 1 Footer Actions */}
                    <div className="flex justify-between items-center pt-3 border-t border-[var(--border-subtle)] mt-2">
                      <button
                        type="button"
                        onClick={() => {
                          setError(null);
                          setStep(0);
                        }}
                        className="flex items-center gap-1 h-8 rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] px-2.5 text-[12px] font-medium text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)] transition-colors cursor-pointer"
                      >
                        <ArrowLeft size={13} />
                        <span>{t('common.back')}</span>
                      </button>

                      <div className="flex items-center gap-2">
                        <button
                          type="button"
                          onClick={onClose}
                          className="h-8 rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] px-3 text-[12px] font-medium text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)] transition-colors cursor-pointer"
                        >
                          {t('common.cancel')}
                        </button>
                        <button
                          type="button"
                          onClick={() => handleSubmit()}
                          disabled={
                            loading ||
                            newPassword.length < 12 ||
                            !confirmPassword ||
                            newPassword !== confirmPassword ||
                            (useNewKeyFile && !newKeyFile.trim())
                          }
                          className="flex h-8 items-center gap-1.5 rounded-[3px] bg-[var(--text-primary)] px-3.5 text-[12px] font-medium text-[var(--bg-base)] transition-all hover:opacity-90 disabled:opacity-40 cursor-pointer shadow-xs"
                        >
                          {loading ? (
                            <>
                              <Loader2 size={13} className="animate-spin" />
                              <span>{t('cmp.changing')}</span>
                            </>
                          ) : (
                            <>
                              <Check size={13} />
                              <span>{t('settings.change_password')}</span>
                            </>
                          )}
                        </button>
                      </div>
                    </div>
                  </motion.div>
                )}
              </AnimatePresence>
            </div>
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}

export default ChangeMasterPasswordModal;
