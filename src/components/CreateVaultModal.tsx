/**
 * CreateVaultModal — Secure vault creation flow
 * 
 * Fields: name, path (with browse), password, confirm
 * Integrated PasswordStrength, validation, backend wiring.
 */

import { useState, useEffect, useRef, useCallback } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { X, Database, FolderOpen, Eye, EyeOff, Loader2, ShieldCheck, KeyRound } from 'lucide-react';
import { PasswordStrength } from './PasswordStrength';
import { isTauri } from '@/lib/backend';
import { useTranslation } from '@/contexts/LanguageContext';
import type { Vault } from '@/types';
import { ActionTooltip } from './ui/tooltip';

interface CreateVaultModalProps {
  open: boolean;
  onClose: () => void;
  onCreated: (vault: Vault) => void;
}

export default function CreateVaultModal({ open, onClose, onCreated }: CreateVaultModalProps) {
  const { t } = useTranslation();
  const [name, setName] = useState('');
  const [path, setPath] = useState('');
  const [pathModified, setPathModified] = useState(false);
  const [password, setPassword] = useState('');
  const [confirmPassword, setConfirmPassword] = useState('');
  const [showPassword, setShowPassword] = useState(false);

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
      const { save } = await import('@tauri-apps/plugin-dialog');
      const selected = await save({
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
        const { save } = await import('@tauri-apps/plugin-dialog');
        const selected = await save({
          title: 'Save New Key File',
          defaultPath: `${name || 'vault'}.key`,
          filters: [{ name: 'Key File (*.key)', extensions: ['key'] }],
        });
        if (selected) {
          setKeyFilePath(selected);
        }
      } else {
        const { open } = await import('@tauri-apps/plugin-dialog');
        const selected = await open({
          title: 'Select Existing Key File',
          multiple: false,
          filters: [{ name: 'Key File (*.key, *.*)', extensions: ['key', '*'] }],
        });
        if (selected) {
          setKeyFilePath(typeof selected === 'string' ? selected : String(selected));
        }
      }
    } catch (e) {
      console.error('Key file browse failed:', e);
    }
  }, [name, generateNewKeyFile]);

  const validate = (checkPath: string): string | null => {
    if (name.trim().length < 2) return 'Vault name must be at least 2 characters';
    if (!checkPath.trim()) return 'Please choose a file location';
    if (password.length < 12) return 'Master password must be at least 12 characters';
    if (password !== confirmPassword) return 'Passwords do not match';
    if (useKeyFile && !keyFilePath.trim()) return 'Please choose or specify a Key File location';
    return null;
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);

    let targetPath = path.trim();

    // If in Tauri and the path is relative or not explicitly modified by the user,
    // force the browse dialog to open so they choose a real location.
    if (isTauri() && (!pathModified || !targetPath || (!targetPath.includes('/') && !targetPath.includes('\\')))) {
      try {
        const { save } = await import('@tauri-apps/plugin-dialog');
        const selected = await save({
          title: 'Choose vault location',
          defaultPath: `${name || 'vault'}.vdb`,
          filters: [{ name: 'Yntra Vault', extensions: ['vdb', 'db'] }],
        });
        if (!selected) {
          setError('You must choose a file location to create the vault');
          return;
        }
        targetPath = selected;
        setPath(selected);
        setPathModified(true);
      } catch (e) {
        console.error('Browse failed:', e);
        setError('Failed to select file location');
        return;
      }
    }

    const validationError = validate(targetPath);
    if (validationError) {
      setError(validationError);
      return;
    }

    setLoading(true);
    try {
      let info;
      if (isTauri()) {
        const { getBackend } = await import('@/lib/backend');
        const backend = await getBackend();

        // If generating a new key file, create it on disk first
        if (useKeyFile && generateNewKeyFile && keyFilePath.trim()) {
          await backend.generateKeyFile(keyFilePath.trim());
        }

        info = await backend.createVault(
          name.trim(),
          password,
          targetPath,
          useKeyFile && keyFilePath.trim() ? keyFilePath.trim() : undefined,
        );
      } else {
        info = { id: crypto.randomUUID(), name: name.trim(), path: targetPath };
      }

      // Save to recent vaults using the real ID
      const recent = JSON.parse(localStorage.getItem('yntra-vault-recent-vaults') || '[]');
      const updated = recent.filter((v: any) => v.id !== info.id && v.path !== info.path);
      const newVault = { id: info.id, name: info.name, path: info.path };
      localStorage.setItem('yntra-vault-recent-vaults', JSON.stringify([newVault, ...updated.slice(0, 9)]));

      onCreated(newVault);
      
      // Security: clear password from state
      setPassword('');
      setConfirmPassword('');
      setName('');
      setPath('');
      setPathModified(false);
      setUseKeyFile(false);
      setKeyFilePath('');
    } catch (err: any) {
      setError(err?.toString() || 'Failed to create vault');
    } finally {
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
          className="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
          onClick={onClose}
        >
          <motion.div
            initial={{ scale: 0.96, opacity: 0 }}
            animate={{ scale: 1, opacity: 1 }}
            exit={{ scale: 0.96, opacity: 0 }}
            transition={{ duration: 0.15 }}
            className="w-[440px] rounded-lg border border-[var(--border)] bg-[var(--bg-base)] shadow-2xl"
            onClick={(e) => e.stopPropagation()}
          >
            {/* Header */}
            <div className="flex items-center justify-between border-b border-[var(--border-subtle)] px-5 py-3.5">
              <div className="flex items-center gap-2.5">
                <Database size={18} className="text-[var(--text-primary)]" />
                <h2 className="text-[16px] font-semibold text-[var(--text-primary)]">{t('create_vault.title')}</h2>
              </div>
              <ActionTooltip content={t('common.close')}>
                <button
                  onClick={onClose}
                  className="rounded-md p-1 text-[var(--text-tertiary)] transition-colors hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"
                >
                  <X size={16} />
                </button>
              </ActionTooltip>
            </div>

            {/* Form */}
            <form onSubmit={handleSubmit} className="flex flex-col gap-4 p-5">
              {/* Vault Name */}
              <div className="flex flex-col gap-1.5">
                <label className="text-[12px] font-medium text-[var(--text-secondary)]">{t('create_vault.vault_name')}</label>
                <input
                  ref={nameRef}
                  type="text"
                  value={name}
                  onChange={(e) => setName(e.target.value)}
                  placeholder={t('create_vault.name_placeholder')}
                  className="h-9 rounded-md border border-[var(--border)] bg-[var(--bg-elevated)] px-3 text-[13px] text-[var(--text-primary)] outline-none placeholder:text-[var(--text-tertiary)] focus:border-[var(--border-focus)]"
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
                    className="h-9 flex-1 rounded-md border border-[var(--border)] bg-[var(--bg-elevated)] px-3 text-[13px] font-mono text-[var(--text-primary)] outline-none placeholder:text-[var(--text-tertiary)] placeholder:font-sans focus:border-[var(--border-focus)]"
                  />
                  {isTauri() && (
                    <ActionTooltip content={t('common.browse')}>
                      <button
                        type="button"
                        onClick={handleBrowse}
                        className="flex h-9 w-9 shrink-0 items-center justify-center rounded-md border border-[var(--border)] bg-[var(--bg-elevated)] text-[var(--text-secondary)] transition-colors hover:bg-[var(--bg-hover)]"
                      >
                        <FolderOpen size={15} />
                      </button>
                    </ActionTooltip>
                  )}
                </div>
              </div>

              {/* Master Password */}
              <div className="flex flex-col gap-1.5">
                <label className="text-[12px] font-medium text-[var(--text-secondary)]">{t('create_vault.master_password')}</label>
                <div className="relative">
                  <input
                    type={showPassword ? 'text' : 'password'}
                    value={password}
                    onChange={(e) => setPassword(e.target.value)}
                    placeholder={t('cmp.min_chars')}
                    className="h-9 w-full rounded-md border border-[var(--border)] bg-[var(--bg-elevated)] px-3 pr-9 font-mono text-[13px] tracking-wide text-[var(--text-primary)] outline-none placeholder:font-sans placeholder:tracking-normal placeholder:text-[var(--text-tertiary)] focus:border-[var(--border-focus)]"
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
                  className={`h-9 w-full rounded-md border bg-[var(--bg-elevated)] px-3 font-mono text-[13px] tracking-wide text-[var(--text-primary)] outline-none placeholder:font-sans placeholder:tracking-normal placeholder:text-[var(--text-tertiary)] focus:border-[var(--border-focus)] ${
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
              <div className="flex flex-col gap-2 rounded-md border border-[var(--border-subtle)] bg-[var(--bg-elevated)] p-3">
                <label className="flex items-center gap-2 cursor-pointer select-none text-[12px] font-medium text-[var(--text-primary)]">
                  <input
                    type="checkbox"
                    checked={useKeyFile}
                    onChange={(e) => {
                      setUseKeyFile(e.target.checked);
                      setError(null);
                    }}
                    className="rounded border-[var(--border)] accent-[var(--accent)]"
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
                        className="h-8 flex-1 rounded border border-[var(--border)] bg-[var(--bg-base)] px-2.5 font-mono text-[12px] text-[var(--text-primary)] outline-none focus:border-[var(--border-focus)]"
                      />
                      {isTauri() && (
                        <button
                          type="button"
                          onClick={handleBrowseKeyFile}
                          className="flex h-8 items-center gap-1 rounded border border-[var(--border)] bg-[var(--bg-base)] px-2.5 text-[11px] font-medium text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"
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
              <div className="flex items-start gap-2 rounded-md bg-[var(--bg-elevated)] px-3 py-2.5">
                <ShieldCheck size={14} className="mt-0.5 shrink-0 text-green-500" />
                <p className="text-[11px] leading-relaxed text-[var(--text-secondary)]">
                  {t('create_vault.security_note')}
                </p>
              </div>

              {/* Error */}
              {error && (
                <div className="rounded-md bg-red-500/10 px-3 py-2 text-[12px] text-red-400">
                  {error}
                </div>
              )}

              {/* Actions */}
              <div className="flex justify-end gap-2 pt-1">
                <button
                  type="button"
                  onClick={onClose}
                  className="h-9 rounded-md px-4 text-[13px] font-medium text-[var(--text-secondary)] transition-colors hover:bg-[var(--bg-hover)]"
                >
                  {t('common.cancel')}
                </button>
                <button
                  type="submit"
                  disabled={loading}
                  className="flex h-9 items-center gap-2 rounded-md bg-[var(--text-primary)] px-4 text-[13px] font-semibold text-[var(--bg-base)] transition-all hover:opacity-90 disabled:opacity-50"
                >
                  {loading ? (
                    <>
                      <Loader2 size={14} className="animate-spin" />
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



