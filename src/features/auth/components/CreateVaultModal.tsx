/**
 * CreateVaultModal — Secure vault creation flow
 * 
 * Fields: name, path (with browse), password, confirm
 * Integrated PasswordStrength, validation, backend wiring.
 */

import { useState, useEffect, useRef, useCallback } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { X, Database, FolderOpen, Eye, EyeOff, Loader2, ShieldCheck, KeyRound } from 'lucide-react';
import { PasswordStrength } from '@/features/audit';
import { isTauri, openFileDialog, saveFileDialog, getBackend } from '@/lib/backend';
import { useTranslation } from '@/contexts/LanguageContext';
import type { Vault } from '@/types';
import { ActionTooltip } from '@/components/ui/tooltip';
import { SecureSecretInput, type SecureSecretInputRef } from '@/components/ui';

export interface CreateVaultModalProps {
  open: boolean;
  onClose: () => void;
  onCreated: (vault: Vault) => void;
}

export function CreateVaultModal({ open, onClose, onCreated }: CreateVaultModalProps) {
  const { t } = useTranslation();
  const [name, setName] = useState('');
  const [path, setPath] = useState('');
  const [pathModified, setPathModified] = useState(false);
  const [password, setPassword] = useState('');
  const [confirmPassword, setConfirmPassword] = useState('');
  const [showPassword, setShowPassword] = useState(false);
  const passInputRef = useRef<SecureSecretInputRef>(null);

  // Key File state
  const [useKeyFile, setUseKeyFile] = useState(false);
  const [keyFilePath, setKeyFilePath] = useState('');
  const [generateNewKeyFile, setGenerateNewKeyFile] = useState(false);

  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const nameRef = useRef<HTMLInputElement>(null);

  // Focus name field on open
  useEffect(() => {
    if (open) {
      setTimeout(() => nameRef.current?.focus(), 100);
    }
  }, [open]);

  // Clear sensitive fields on close
  useEffect(() => {
    if (!open) {
      setPassword('');
      setConfirmPassword('');
      setError(null);
      setShowPassword(false);
      setName('');
      setPath('');
      setPathModified(false);
      setUseKeyFile(false);
      setKeyFilePath('');
      setGenerateNewKeyFile(false);
    }
  }, [open]);

  // Esc to close
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && open) onClose();
    };
    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, [open, onClose]);

  // Auto-generate path from name
  useEffect(() => {
    if (!pathModified) {
      const safeName = name ? name.toLowerCase().replace(/[^a-z0-9]/g, '-').replace(/-+/g, '-') : '';
      if (safeName) {
        if (isTauri()) {
          setPath(`${safeName}.vdb`);
        } else {
          setPath(`~/.yntra-vault/${safeName}.db`);
        }
      } else {
        setPath('');
      }
    }
  }, [name, pathModified]);

  const handleBrowse = useCallback(async () => {
    if (!isTauri()) return;
    try {
      const selected = await saveFileDialog({
        title: 'Choose vault location',
        defaultPath: `${name || 'vault'}.vdb`,
        filters: [{ name: 'Yntra Vault', extensions: ['vdb', 'db'] }],
      });
      if (selected) {
        setPath(selected);
        setPathModified(true);
      }
    } catch (e) {
      console.error('Browse failed:', e);
    }
  }, [name]);

  const handleBrowseKeyFile = useCallback(async () => {
    if (!isTauri()) return;
    try {
      if (generateNewKeyFile) {
        const selected = await saveFileDialog({
          title: 'Save New Key File',
          defaultPath: `${name || 'vault'}.key`,
          filters: [{ name: 'Key File (*.key)', extensions: ['key'] }],
        });
        if (selected) {
          setKeyFilePath(selected);
        }
      } else {
        const selected = await openFileDialog({
          title: 'Select Existing Key File',
          multiple: false,
          filters: [{ name: 'Key File (*.key, *.*)', extensions: ['key', '*'] }],
        });
        if (selected) {
          setKeyFilePath(typeof selected === 'string' ? selected : String(selected[0]));
        }
      }
    } catch (e) {
      console.error('Key file browse failed:', e);
    }
  }, [name, generateNewKeyFile]);

  const validate = (checkPath: string, passLength: number): string | null => {
    if (name.trim().length < 2) return t('create_vault.err_name_short') || 'Vault name must be at least 2 characters';
    if (!checkPath.trim()) return t('create_vault.err_choose_location') || 'Please choose a file location';
    if (passLength < 12) return t('create_vault.err_pass_length') || 'Master password must be at least 12 characters';
    if (!confirmPassword) return t('create_vault.err_confirm_pass') || 'Please confirm your master password';
    if (password !== confirmPassword) return t('create_vault.err_pass_mismatch');
    if (useKeyFile && !keyFilePath.trim()) return t('create_vault.err_keyfile_required') || 'Please choose or specify a Key File location';
    return null;
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);

    let targetPath = path.trim();
    const secretBytes = passInputRef.current?.getSecretBytes();
    const passBytes = secretBytes && secretBytes.length > 0 ? secretBytes : new TextEncoder().encode(password);

    // If in Tauri and the path is relative or not explicitly modified by the user,
    // force the browse dialog to open so they choose a real location.
    if (isTauri() && (!pathModified || !targetPath || (!targetPath.includes('/') && !targetPath.includes('\\')))) {
      try {
        const selected = await saveFileDialog({
          title: 'Choose vault location',
          defaultPath: `${name || 'vault'}.vdb`,
          filters: [{ name: 'Yntra Vault', extensions: ['vdb', 'db'] }],
        });
        if (!selected) {
          setError(t('create_vault.err_choose_location') || 'You must choose a file location to create the vault');
          return;
        }
        targetPath = selected;
        setPath(selected);
        setPathModified(true);
      } catch (e) {
        console.error('Browse failed:', e);
        setError(t('create_vault.err_choose_location') || 'Failed to select file location');
        return;
      }
    }

    const validationError = validate(targetPath, passBytes.length);
    if (validationError) {
      setError(validationError);
      return;
    }

    setLoading(true);
    try {
      let info;
      if (isTauri()) {
        const backend = await getBackend();

        // If generating a new key file, create it on disk first
        if (useKeyFile && generateNewKeyFile && keyFilePath.trim()) {
          await backend.generateKeyFile(keyFilePath.trim());
        }

        info = await backend.createVaultBytes(
          name.trim(),
          passBytes,
          targetPath,
          useKeyFile && keyFilePath.trim() ? keyFilePath.trim() : undefined,
        );
      } else {
        info = { id: crypto.randomUUID(), name: name.trim(), path: targetPath };
      }

      // Do not persist keyfile paths in localStorage to preserve 2FA factor isolation
      localStorage.removeItem('yntra-vault-keyfiles');

      const recent = JSON.parse(localStorage.getItem('yntra-vault-recent-vaults') || '[]');
      const updated = recent.filter((v: any) => v.id !== info.id && v.path !== info.path);
      const newVault = { id: info.id, name: info.name, path: info.path };
      localStorage.setItem('yntra-vault-recent-vaults', JSON.stringify([newVault, ...updated.slice(0, 9)]));

      // Trigger the new-vault tutorial on first open
      localStorage.setItem('yntra-vault-show-tutorial', 'true');

      onCreated(newVault);
      
      // Security: clear password from state
      setPassword('');
      setConfirmPassword('');
      setName('');
      setPath('');
      setKeyFilePath('');
      setUseKeyFile(false);
      setGenerateNewKeyFile(true);
      onClose();
    } catch (err: any) {
      setError(err?.toString() || 'Failed to create vault');
    } finally {
      passBytes.fill(0);
      passInputRef.current?.clearSecretBytes();
      setLoading(false);
    }
  };

  return (
    <AnimatePresence>
      {open && (
        <motion.div
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          className="fixed inset-0 z-50 flex items-start sm:items-center justify-center overflow-y-auto bg-black/50 select-none p-3 sm:p-4 touch-pan-y"
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
            {/* Header */}
            <div className="flex items-center justify-between border-b border-[var(--border-subtle)] px-5 py-3.5 bg-[var(--bg-base)]">
              <div className="flex items-center gap-2.5">
                <div className="flex h-7 w-7 items-center justify-center rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] text-[var(--text-secondary)]">
                  <Database size={14} />
                </div>
                <div>
                  <h2 className="text-[14px] font-medium text-[var(--text-primary)] leading-tight">{t('create_vault.title')}</h2>
                  <p className="text-[11px] text-[var(--text-tertiary)]">{t('create_vault.subtitle')}</p>
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

            {/* Form */}
            <form onSubmit={handleSubmit} className="flex flex-col gap-4 p-5 flex-1 min-h-0 overflow-y-auto touch-pan-y">
              {/* Vault Name */}
              <div className="flex flex-col gap-1.5">
                <label className="text-[12px] font-medium text-[var(--text-secondary)]">{t('create_vault.vault_name')}</label>
                <input
                  ref={nameRef}
                  type="text"
                  value={name}
                  onChange={(e) => setName(e.target.value)}
                  placeholder={t('create_vault.name_placeholder')}
                  className="h-8 rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] px-2.5 text-[12px] text-[var(--text-primary)] outline-none placeholder:text-[var(--text-tertiary)] focus:border-[var(--border-focus)]"
                />
              </div>

              {/* File Path */}
              <div className="flex flex-col gap-1.5">
                <label className="text-[12px] font-medium text-[var(--text-secondary)]">{t('create_vault.location')}</label>
                <div className="flex gap-1.5">
                  <input
                    type="text"
                    value={path}
                    onChange={(e) => {
                      setPath(e.target.value);
                      setPathModified(true);
                    }}
                    placeholder={t('create_vault.location_ph')}
                    className="h-8 flex-1 rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] px-2.5 text-[12px] font-mono text-[var(--text-primary)] outline-none placeholder:text-[var(--text-tertiary)] placeholder:font-sans focus:border-[var(--border-focus)]"
                  />
                  {isTauri() && (
                    <ActionTooltip content={t('common.browse')}>
                      <button
                        type="button"
                        onClick={handleBrowse}
                        className="flex h-8 w-8 shrink-0 items-center justify-center rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] text-[var(--text-secondary)] transition-colors hover:bg-[var(--bg-hover)] cursor-pointer"
                      >
                        <FolderOpen size={14} />
                      </button>
                    </ActionTooltip>
                  )}
                </div>
              </div>

              {/* Master Password */}
              <div className="flex flex-col gap-1.5">
                <label className="text-[12px] font-medium text-[var(--text-secondary)]">{t('create_vault.master_password')}</label>
                <div className="relative">
                  <SecureSecretInput
                    ref={passInputRef}
                    value={password}
                    onChange={setPassword}
                    show={showPassword}
                    placeholder={t('cmp.min_chars')}
                    className="w-full pr-9"
                  />
                  <ActionTooltip content={showPassword ? t('login.hide_password') : t('login.show_password')}>
                    <button
                      type="button"
                      onClick={() => setShowPassword(!showPassword)}
                      className="absolute right-2 top-1/2 -translate-y-1/2 text-[var(--text-tertiary)] hover:text-[var(--text-primary)]"
                    >
                      {showPassword ? <EyeOff size={14} /> : <Eye size={14} />}
                    </button>
                  </ActionTooltip>
                </div>
                {password.length > 0 && <PasswordStrength password={password} compact />}
              </div>

              {/* Confirm Password */}
              <div className="flex flex-col gap-1.5">
                <label className="text-[12px] font-medium text-[var(--text-secondary)]">{t('create_vault.confirm_password')}</label>
                <input
                  type={showPassword ? 'text' : 'password'}
                  value={confirmPassword}
                  onChange={(e) => setConfirmPassword(e.target.value)}
                  placeholder={t('create_vault.reenter_pass_ph')}
                  className={`h-8 w-full rounded-[3px] border bg-[var(--bg-base)] px-2.5 font-mono text-[12px] tracking-wide text-[var(--text-primary)] outline-none placeholder:font-sans placeholder:tracking-normal placeholder:text-[var(--text-tertiary)] focus:border-[var(--border-focus)] ${
                    confirmPassword && confirmPassword !== password
                      ? 'border-[var(--destructive)]'
                      : 'border-[var(--border)]'
                  }`}
                />
                {confirmPassword && confirmPassword !== password && (
                  <span className="text-[11px] text-[var(--destructive)]">{t('create_vault.err_pass_mismatch')}</span>
                )}
              </div>

              {/* Key File Option */}
              <div className="flex flex-col gap-2 rounded-[3px] border border-[var(--border-subtle)] bg-[var(--bg-base)] p-3">
                <label className="flex items-center gap-2 cursor-pointer select-none text-[12px] font-medium text-[var(--text-primary)]">
                  <input
                    type="checkbox"
                    checked={useKeyFile}
                    onChange={(e) => {
                      setUseKeyFile(e.target.checked);
                      setError(null);
                    }}
                    className="rounded-[2px] border-[var(--border)] accent-[var(--accent)]"
                  />
                  <KeyRound size={14} className="text-[var(--text-secondary)]" />
                  <span>{t('create_vault.enable_keyfile')}</span>
                </label>

                {useKeyFile && (
                  <div className="mt-1 flex flex-col gap-2 pl-6">
                    <div className="flex items-center gap-4 text-[11px] text-[var(--text-secondary)]">
                      <label className="flex items-center gap-1.5 cursor-pointer">
                        <input
                          type="radio"
                          name="keyFileMode"
                          checked={!generateNewKeyFile}
                          onChange={() => setGenerateNewKeyFile(false)}
                          className="accent-[var(--accent)]"
                        />
                        <span>{t('create_vault.use_existing_keyfile')}</span>
                      </label>
                      <label className="flex items-center gap-1.5 cursor-pointer">
                        <input
                          type="radio"
                          name="keyFileMode"
                          checked={generateNewKeyFile}
                          onChange={() => setGenerateNewKeyFile(true)}
                          className="accent-[var(--accent)]"
                        />
                        <span>{t('create_vault.gen_new_keyfile')}</span>
                      </label>
                    </div>

                    <div className="flex gap-1.5">
                      <input
                        type="text"
                        value={keyFilePath}
                        onChange={(e) => setKeyFilePath(e.target.value)}
                        placeholder={generateNewKeyFile ? t('create_vault.save_keyfile_ph') : t('create_vault.exist_keyfile_ph')}
                        className="h-8 flex-1 rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] px-2.5 font-mono text-[12px] text-[var(--text-primary)] outline-none focus:border-[var(--border-focus)]"
                      />
                      {isTauri() && (
                        <button
                          type="button"
                          onClick={handleBrowseKeyFile}
                          className="flex h-8 items-center gap-1 rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] px-2.5 text-[11px] font-medium text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)] cursor-pointer"
                        >
                          <FolderOpen size={12} />
                          {t('common.browse')}
                        </button>
                      )}
                    </div>
                  </div>
                )}
              </div>

              {/* Security Note */}
              <div className="flex items-start gap-2 rounded-[3px] border border-[var(--border-subtle)] bg-[var(--bg-base)] px-3 py-2.5">
                <ShieldCheck size={14} className="mt-0.5 shrink-0 text-[var(--text-secondary)]" />
                <p className="text-[11px] leading-relaxed text-[var(--text-secondary)]">
                  {t('create_vault.security_note')}
                </p>
              </div>

              {/* Error */}
              {error && (
                <div className="rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] px-3 py-2 text-[12px] text-[var(--destructive)] font-medium">
                  {error}
                </div>
              )}

              {/* Actions */}
              <div className="flex justify-end gap-2 pt-1 border-t border-[var(--border-subtle)]">
                <button
                  type="button"
                  onClick={onClose}
                  className="h-8 rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] px-3 text-[12px] font-medium text-[var(--text-secondary)] transition-colors hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)] cursor-pointer"
                >
                  {t('common.cancel')}
                </button>
                <button
                  type="submit"
                  disabled={loading}
                  className="flex h-8 items-center gap-1.5 rounded-[3px] bg-[var(--text-primary)] px-3.5 text-[12px] font-semibold text-[var(--bg-base)] transition-opacity hover:opacity-90 disabled:opacity-50 cursor-pointer"
                >
                  {loading ? (
                    <>
                      <Loader2 size={13} className="animate-spin" />
                      {t('create_vault.creating')}
                    </>
                  ) : (
                    t('create_vault.create_btn')
                  )}
                </button>
              </div>
            </form>
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}

export default CreateVaultModal;
