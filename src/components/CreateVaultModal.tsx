/**
 * CreateVaultModal — Secure vault creation flow
 * 
 * Fields: name, path (with browse), password, confirm
 * Integrated PasswordStrength, validation, backend wiring.
 */

import { useState, useEffect, useRef, useCallback } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { X, Database, FolderOpen, Eye, EyeOff, Loader2, ShieldCheck } from 'lucide-react';
import { PasswordStrength } from './PasswordStrength';
import { isTauri } from '@/lib/backend';

interface CreateVaultModalProps {
  open: boolean;
  onClose: () => void;
  onCreated: (name: string, path: string) => void;
}

export default function CreateVaultModal({ open, onClose, onCreated }: CreateVaultModalProps) {
  const [name, setName] = useState('');
  const [path, setPath] = useState('');
  const [password, setPassword] = useState('');
  const [confirmPassword, setConfirmPassword] = useState('');
  const [showPassword, setShowPassword] = useState(false);
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
    if (name && !path) {
      const safeName = name.toLowerCase().replace(/[^a-z0-9]/g, '-').replace(/-+/g, '-');
      if (isTauri()) {
        setPath(`${safeName}.vdb`);
      } else {
        setPath(`~/.yntra-vault/${safeName}.db`);
      }
    }
  }, [name]);

  const handleBrowse = useCallback(async () => {
    if (!isTauri()) return;
    try {
      const { save } = await import('@tauri-apps/plugin-dialog');
      const selected = await save({
        title: 'Choose vault location',
        defaultPath: `${name || 'vault'}.vdb`,
        filters: [{ name: 'Yntra Vault', extensions: ['vdb', 'db'] }],
      });
      if (selected) setPath(selected);
    } catch (e) {
      console.error('Browse failed:', e);
    }
  }, [name]);

  const validate = (): string | null => {
    if (name.trim().length < 2) return 'Vault name must be at least 2 characters';
    if (!path.trim()) return 'Please choose a file location';
    if (password.length < 12) return 'Master password must be at least 12 characters';
    if (password !== confirmPassword) return 'Passwords do not match';
    return null;
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);

    const validationError = validate();
    if (validationError) {
      setError(validationError);
      return;
    }

    setLoading(true);
    try {
      if (isTauri()) {
        const { getBackend } = await import('@/lib/backend');
        const backend = await getBackend();
        await backend.createVault(name.trim(), password, path.trim());
      }

      // Save to recent vaults
      const recent = JSON.parse(localStorage.getItem('yntra-vault-recent-vaults') || '[]');
      const newVault = { id: crypto.randomUUID(), name: name.trim(), path: path.trim() };
      localStorage.setItem('yntra-vault-recent-vaults', JSON.stringify([newVault, ...recent.slice(0, 9)]));

      onCreated(name.trim(), path.trim());
      
      // Security: clear password from state
      setPassword('');
      setConfirmPassword('');
      setName('');
      setPath('');
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
                <h2 className="text-[16px] font-semibold text-[var(--text-primary)]">Create New Vault</h2>
              </div>
              <button
                onClick={onClose}
                className="rounded-md p-1 text-[var(--text-tertiary)] transition-colors hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"
              >
                <X size={16} />
              </button>
            </div>

            {/* Form */}
            <form onSubmit={handleSubmit} className="flex flex-col gap-4 p-5">
              {/* Vault Name */}
              <div className="flex flex-col gap-1.5">
                <label className="text-[12px] font-medium text-[var(--text-secondary)]">Vault Name</label>
                <input
                  ref={nameRef}
                  type="text"
                  value={name}
                  onChange={(e) => setName(e.target.value)}
                  placeholder="Personal Vault"
                  className="h-9 rounded-md border border-[var(--border)] bg-[var(--bg-elevated)] px-3 text-[13px] text-[var(--text-primary)] outline-none placeholder:text-[var(--text-tertiary)] focus:border-[var(--border-focus)]"
                />
              </div>

              {/* File Path */}
              <div className="flex flex-col gap-1.5">
                <label className="text-[12px] font-medium text-[var(--text-secondary)]">File Location</label>
                <div className="flex gap-1.5">
                  <input
                    type="text"
                    value={path}
                    onChange={(e) => setPath(e.target.value)}
                    placeholder="vault.vdb"
                    className="h-9 flex-1 rounded-md border border-[var(--border)] bg-[var(--bg-elevated)] px-3 text-[13px] font-mono text-[var(--text-primary)] outline-none placeholder:text-[var(--text-tertiary)] placeholder:font-sans focus:border-[var(--border-focus)]"
                  />
                  {isTauri() && (
                    <button
                      type="button"
                      onClick={handleBrowse}
                      className="flex h-9 w-9 shrink-0 items-center justify-center rounded-md border border-[var(--border)] bg-[var(--bg-elevated)] text-[var(--text-secondary)] transition-colors hover:bg-[var(--bg-hover)]"
                    >
                      <FolderOpen size={15} />
                    </button>
                  )}
                </div>
              </div>

              {/* Master Password */}
              <div className="flex flex-col gap-1.5">
                <label className="text-[12px] font-medium text-[var(--text-secondary)]">Master Password</label>
                <div className="relative">
                  <input
                    type={showPassword ? 'text' : 'password'}
                    value={password}
                    onChange={(e) => setPassword(e.target.value)}
                    placeholder="Minimum 12 characters"
                    className="h-9 w-full rounded-md border border-[var(--border)] bg-[var(--bg-elevated)] px-3 pr-9 font-mono text-[13px] tracking-wide text-[var(--text-primary)] outline-none placeholder:font-sans placeholder:tracking-normal placeholder:text-[var(--text-tertiary)] focus:border-[var(--border-focus)]"
                  />
                  <button
                    type="button"
                    onClick={() => setShowPassword(!showPassword)}
                    className="absolute right-2 top-1/2 -translate-y-1/2 text-[var(--text-tertiary)] hover:text-[var(--text-primary)]"
                  >
                    {showPassword ? <EyeOff size={14} /> : <Eye size={14} />}
                  </button>
                </div>
                {password.length > 0 && <PasswordStrength password={password} compact />}
              </div>

              {/* Confirm Password */}
              <div className="flex flex-col gap-1.5">
                <label className="text-[12px] font-medium text-[var(--text-secondary)]">Confirm Password</label>
                <input
                  type={showPassword ? 'text' : 'password'}
                  value={confirmPassword}
                  onChange={(e) => setConfirmPassword(e.target.value)}
                  placeholder="Re-enter master password"
                  className={`h-9 w-full rounded-md border bg-[var(--bg-elevated)] px-3 font-mono text-[13px] tracking-wide text-[var(--text-primary)] outline-none placeholder:font-sans placeholder:tracking-normal placeholder:text-[var(--text-tertiary)] focus:border-[var(--border-focus)] ${
                    confirmPassword && confirmPassword !== password
                      ? 'border-[var(--destructive)]'
                      : 'border-[var(--border)]'
                  }`}
                />
                {confirmPassword && confirmPassword !== password && (
                  <span className="text-[11px] text-[var(--destructive)]">Passwords do not match</span>
                )}
              </div>

              {/* Security Note */}
              <div className="flex items-start gap-2 rounded-md bg-[var(--bg-elevated)] px-3 py-2.5">
                <ShieldCheck size={14} className="mt-0.5 shrink-0 text-green-500" />
                <p className="text-[11px] leading-relaxed text-[var(--text-secondary)]">
                  Your vault is encrypted with Argon2id + XChaCha20-Poly1305 + AES-256-GCM.
                  The master password never leaves your device.
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
                  Cancel
                </button>
                <button
                  type="submit"
                  disabled={loading}
                  className="flex h-9 items-center gap-2 rounded-md bg-[var(--text-primary)] px-4 text-[13px] font-semibold text-[var(--bg-base)] transition-all hover:opacity-90 disabled:opacity-50"
                >
                  {loading ? (
                    <>
                      <Loader2 size={14} className="animate-spin" />
                      Creating...
                    </>
                  ) : (
                    'Create Vault'
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



