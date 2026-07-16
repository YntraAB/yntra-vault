/**
 * ChangeMasterPasswordModal — Secure master password change
 */

import { useState, useEffect } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { X, Loader2, ShieldCheck } from 'lucide-react';
import { PasswordStrength } from './PasswordStrength';
import { useAppState } from '@/contexts/AppStateContext';
import { isTauri, getBackend } from '@/lib/backend';

interface ChangeMasterPasswordModalProps {
  open: boolean;
  onClose: () => void;
}

export default function ChangeMasterPasswordModal({ open, onClose }: ChangeMasterPasswordModalProps) {
  const { addToast } = useAppState();
  const [currentPassword, setCurrentPassword] = useState('');
  const [newPassword, setNewPassword] = useState('');
  const [confirmPassword, setConfirmPassword] = useState('');
  const [showPasswords, setShowPasswords] = useState(false);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!open) {
      setCurrentPassword('');
      setNewPassword('');
      setConfirmPassword('');
      setError(null);
      setShowPasswords(false);
    }
  }, [open]);

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && open) onClose();
    };
    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, [open, onClose]);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);

    if (!currentPassword) { setError('Enter your current password'); return; }
    if (newPassword.length < 12) { setError('New password must be at least 12 characters'); return; }
    if (newPassword !== confirmPassword) { setError('Passwords do not match'); return; }
    if (newPassword === currentPassword) { setError('New password must be different'); return; }

    setLoading(true);
    try {
      if (isTauri()) {
        const backend = await getBackend();
        await backend.changeMasterPassword(currentPassword, newPassword);
      }
      addToast({ message: 'Master password changed', type: 'success' });
      onClose();
    } catch (err: any) {
      setError(err?.toString() || 'Failed to change password');
    } finally {
      setLoading(false);
      setCurrentPassword('');
      setNewPassword('');
      setConfirmPassword('');
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
            className="w-[400px] rounded-lg border border-[var(--border)] bg-[var(--bg-base)] shadow-2xl"
            onClick={(e) => e.stopPropagation()}
          >
            <div className="flex items-center justify-between border-b border-[var(--border-subtle)] px-5 py-3.5">
              <div className="flex items-center gap-2">
                <ShieldCheck size={18} className="text-[var(--text-primary)]" />
                <h2 className="text-[16px] font-semibold text-[var(--text-primary)]">Change Master Password</h2>
              </div>
              <button onClick={onClose} className="rounded-md p-1 text-[var(--text-tertiary)] hover:bg-[var(--bg-hover)]">
                <X size={16} />
              </button>
            </div>

            <form onSubmit={handleSubmit} className="flex flex-col gap-4 p-5">
              <PasswordField
                label="Current Password"
                value={currentPassword}
                onChange={setCurrentPassword}
                show={showPasswords}
                placeholder="Enter current password"
              />

              <PasswordField
                label="New Password"
                value={newPassword}
                onChange={setNewPassword}
                show={showPasswords}
                placeholder="Minimum 12 characters"
              />
              {newPassword && <PasswordStrength password={newPassword} compact />}

              <PasswordField
                label="Confirm New Password"
                value={confirmPassword}
                onChange={setConfirmPassword}
                show={showPasswords}
                placeholder="Re-enter new password"
                mismatch={!!confirmPassword && confirmPassword !== newPassword}
              />

              <label className="flex items-center gap-2 text-[12px] text-[var(--text-secondary)]">
                <input
                  type="checkbox"
                  checked={showPasswords}
                  onChange={(e) => setShowPasswords(e.target.checked)}
                  className="accent-[var(--accent)] h-3.5 w-3.5"
                />
                Show passwords
              </label>

              {error && (
                <div className="rounded-md bg-red-500/10 px-3 py-2 text-[12px] text-red-400">{error}</div>
              )}

              <div className="flex justify-end gap-2 pt-1">
                <button type="button" onClick={onClose} className="h-9 rounded-md px-4 text-[13px] font-medium text-[var(--text-secondary)] hover:bg-[var(--bg-hover)]">
                  Cancel
                </button>
                <button type="submit" disabled={loading} className="flex h-9 items-center gap-2 rounded-md bg-[var(--text-primary)] px-4 text-[13px] font-semibold text-[var(--bg-base)] hover:opacity-90 disabled:opacity-50">
                  {loading ? <><Loader2 size={14} className="animate-spin" /> Changing...</> : 'Change Password'}
                </button>
              </div>
            </form>
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}

function PasswordField({ label, value, onChange, show, placeholder, mismatch }: {
  label: string; value: string; onChange: (v: string) => void;
  show: boolean; placeholder: string; mismatch?: boolean;
}) {
  return (
    <div className="flex flex-col gap-1.5">
      <label className="text-[12px] font-medium text-[var(--text-secondary)]">{label}</label>
      <input
        type={show ? 'text' : 'password'}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder={placeholder}
        className={`h-9 rounded-md border bg-[var(--bg-elevated)] px-3 font-mono text-[13px] tracking-wide text-[var(--text-primary)] outline-none placeholder:font-sans placeholder:tracking-normal placeholder:text-[var(--text-tertiary)] focus:border-[var(--border-focus)] ${
          mismatch ? 'border-[var(--destructive)]' : 'border-[var(--border)]'
        }`}
      />
      {mismatch && <span className="text-[11px] text-[var(--destructive)]">Passwords do not match</span>}
    </div>
  );
}

